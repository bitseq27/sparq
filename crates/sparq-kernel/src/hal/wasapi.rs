//! The WASAPI backends: exclusive (event-driven) and shared — WO-006 task 3, ADR-004's
//! Windows-primary decision made real.
//!
//! # Shape
//!
//! ```text
//! open()    [control thread]  COM(MTA) → resolve endpoint → negotiate format (probe ladder)
//!                             → Initialize (exclusive: alignment two-step) → SetEventHandle
//!                             → GetService(render) → GetBufferSize → allocate+pre-touch
//! start()   [control thread]  spawn pump; wait (bounded) for Running/Failed/Removed
//! pump      [pump thread]     COM(MTA) → MMCSS+priority → prime FIFO → preroll → Start()
//!                             → loop { wait(event) → padding → write → refill }
//! stop()    [control thread]  flag + SetEvent → join → client.Stop() happened in the pump
//! ```
//!
//! All COM objects are created in a **persistent per-thread MTA** on the control thread
//! ([`ensure_control_com`]) and used by the pump thread, which initialises MTA as well. MTA
//! interfaces need no marshalling between MTA threads; ownership *moves* to the pump and returns
//! on `join`, so exactly one thread holds them at any moment. The control apartment outlives
//! every object because its init is never released (documented, deliberate: process-lifetime COM).
//!
//! # Why the vtables are hand-written
//!
//! `windows-sys` (the memory-light bindings this crate can compile under CI's 1 GB budget) ships
//! free functions and POD types but no COM interface definitions; the high-level `windows` crate
//! does, but exceeds that budget. So the seven interfaces below are declared directly against the
//! **ABI-frozen** layout of the Windows SDK headers (`mmdeviceapi.h`, `audioclient.h`,
//! `propsys.h`) — method order has been contract-stable since Vista and is exactly what
//! allowlist entry 1 means by "raw interface calls". Every slot carries a comment naming its SDK
//! method; the typed wrappers in `impl Com<_>` blocks are the only places the vtables are called.
//!
//! # Period vs block: the FIFO
//!
//! The device delivers/consumes its **own** period P (exclusive: what we negotiated, possibly
//! adjusted by the alignment two-step; shared: the audio engine's, typically 10 ms). sparq's
//! callback contract is a fixed **block** B. They are not equal and must not be forced equal —
//! so the pump keeps a pre-allocated power-of-two FIFO between them: per event it writes
//! `P - padding` frames to the device (converting f32→i16/i32 when the negotiated format demands it),
//! then runs the callback until the FIFO is back above its cushion. The callback only ever sees
//! exactly B frames; the device only ever sees exactly what it asked for. Starvation (FIFO short
//! at write time) is counted as an xrun and padded with silence — audible truth, counted truth.
//!
//! In event-driven EXCLUSIVE the API pins the two numbers together — `hnsPeriodicity` must be
//! nonzero and **equal to** `hnsBufferDuration` (`IAudioClient::Initialize`'s documented rule,
//! enforced with `AUDCLNT_E_BUFDURATION_PERIOD_NOT_EQUAL`) — so an exclusive stream can never
//! bank more than one period: there is no deeper device buffer to hide a driver's event hiccup
//! behind, and the period itself is the only headroom lever. A driver that ACCEPTED a cadence
//! and then stalls on it is only observable at runtime, which is why the drift metric counts
//! frames the device ACCEPTED against the wall (#76's re-derivation — `IAudioClock::GetPosition`
//! advances in buffer-sized steps per event tick on these endpoints and is not a clock; see
//! `docs/hal/windows-notes.md` §4b/§4e) and why the play/soak verdicts treat a suspect drift
//! as a failure, not a curiosity.
//!
//! # Formats
//!
//! Exclusive: a four-rung ladder, all in the extensible shape (the documented exclusive
//! contract): **f32** — the format sparq thinks in, no conversion on the hot path — then
//! **i24-in-32**, **i32**, **i16** (defect #77: USB DAC drivers commonly refuse IEEE-float in
//! exclusive; 24-in-32 is the native currency of USB audio, and the Behringer UMC 204HD 192k
//! on SATURN refused f32 exclusive while being fully exclusive-capable). The pump converts for
//! the integer rungs; a 24-in-32 stream is left-justified in its container, so the f32→i32
//! arithmetic serves both 32-bit rungs unchanged and the DAC ignores the low 8 bits.
//! Shared: probe ladder f32@(requested rate) → f32@(mix rate) → the mix format itself. If the
//! mix format is 16-bit PCM (very common — the Phase A lesson), the pump converts; anything else
//! is refused honestly at `open` rather than guessed at.
//!
//! # Failure semantics (WO-006 acceptance)
//!
//! `AUDCLNT_E_DEVICE_INVALIDATED` (unplug, driver reset) anywhere in the pump → state `Removed`,
//! pump exits, `stop()` still joins cleanly, `start()` refuses with guidance. Exclusive-mode
//! hijack / device in use → `Busy` with the settings path in the hint. `BUFFER_SIZE_NOT_ALIGNED`
//! → the documented two-step retry, transparently. Every HRESULT is classified once, in
//! [`classify`], and the classification is what the CLI prints.
//!
//! # Not yet (WO-006 increment 2, documented not hidden)
//!
//! * Full duplex (capture): the trait expresses it, the null backend proves the code path, this
//!   backend returns `Unsupported` until the capture-endpoint pairing is built.
//! * Round-trip *measurement* (loopback impulse): `measured_roundtrip_frames` stays `None` —
//!   the estimate in [`LatencyReport`] says "est" until it is measured.
//! * ASIO: `hal/asio.rs`, allowlist entry 2.

// Allowlisted: docs/unsafe-allowlist.md entry 1 (`sparq-kernel/src/hal/wasapi.rs`).
// The invariants, per the allowlist row: device buffer pointers from GetBuffer are valid for
// exactly `frame_count` frames of the negotiated format and only until ReleaseBuffer; the
// channel count used to interpret them is the negotiated one; COM is initialised (MTA) on every
// thread that touches an interface; interface ownership moves to exactly one thread at a time
// (the `Com<T>` wrapper is the sole owner and Releases exactly once in Drop).
// Each `unsafe` block below restates the specific invariant it relies on. Hardware-tested by the
// WO-006 acceptance runs on the stage machine (scripts/hal.bat, scripts/soak.bat) — this module
// cannot be exercised on a device-less CI runner, which is why `cargo clippy` for the Windows
// target is a CI gate (.github/workflows/ci.yml: windows-crosscheck).
#![allow(unsafe_code)]
// This module is only compiled on Windows with the feature enabled (see hal/mod.rs). The pump
// legitimately measures time and waits on kernel objects — that is the device-facing harness
// side, never the callback side (the callback receives only a FrameCtx, as everywhere in the HAL).
#![allow(clippy::disallowed_methods)]
// The vtable structs mirror the Windows SDK headers field-for-field (PascalCase methods,
// `lpVtbl`): keeping the SDK spelling makes every slot auditable against the header it came
// from, which is worth more than local naming style at an ABI boundary.
#![allow(non_snake_case)]

use std::ffi::c_void;
use std::ptr;
use std::sync::atomic::{AtomicBool, AtomicI64, AtomicIsize, AtomicU8, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

use windows_sys::core::{GUID, HRESULT, PCWSTR, PWSTR};
use windows_sys::Win32::Foundation::{
    CloseHandle, HANDLE, PROPERTYKEY, RPC_E_CHANGED_MODE, WAIT_OBJECT_0, WAIT_TIMEOUT,
};
use windows_sys::Win32::Media::Audio::{
    eMultimedia, eRender, EDataFlow, ERole, AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED,
    AUDCLNT_E_DEVICE_INVALIDATED, AUDCLNT_E_DEVICE_IN_USE, AUDCLNT_E_EXCLUSIVE_MODE_NOT_ALLOWED,
    AUDCLNT_E_INVALID_DEVICE_PERIOD, AUDCLNT_E_UNSUPPORTED_FORMAT, AUDCLNT_SHAREMODE_EXCLUSIVE,
    AUDCLNT_SHAREMODE_SHARED, AUDCLNT_STREAMFLAGS_EVENTCALLBACK, DEVICE_STATE_ACTIVE, WAVEFORMATEX,
    WAVEFORMATEXTENSIBLE, WAVEFORMATEXTENSIBLE_0,
};
use windows_sys::Win32::System::Com::StructuredStorage::{PropVariantClear, PROPVARIANT};
use windows_sys::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CoTaskMemFree, CoUninitialize, CLSCTX_ALL,
    COINIT_MULTITHREADED, STGM_READ,
};
use windows_sys::Win32::System::Threading::{CreateEventW, SetEvent, WaitForSingleObject};
use windows_sys::Win32::System::Variant::VT_LPWSTR;

use crate::alloc::CountingGuard;
use crate::block::{BlockContext, BlockId};
use crate::clock::Clock;
use crate::device::StreamConfig;
use crate::hal::diag::DiagRecorder;
use crate::hal::{
    state_from_u8, state_to_u8, AudioStream, BackendKind, Capabilities, DeviceId, DeviceInfo,
    Fault, FrameCtx, HalBackend, HalError, LatencyReport, OpenOptions, ProcessFn, RateRange,
    StreamErrorReport, StreamState,
};
use crate::rt::thread::{pre_touch, RtThreadGuard};

// --------------------------------------------------------------------------- SDK constants
// Values below are the published, ABI-frozen constants from the Windows SDK headers
// (mmdeviceapi.h, audioclient.h, ksmedia.h, propkey.h). They are written out here because
// `windows-sys` ships free functions but not CLSID/IID/PKEY data — and because a hand-checked
// constant with its source named is more auditable than a re-export nobody reads.

/// `GUID::from_u128` is the least typo-prone spelling of a 128-bit constant; the values below
/// are the published SDK uuids, checked against the headers and pinned by
/// `guid_constants_match_the_published_sdk_values`.
const fn guid(u: u128) -> GUID {
    GUID::from_u128(u)
}

/// `{BCDE0395-E52F-467C-8E3D-C4579291692E}` (mmdeviceapi.h).
const CLSID_MMDEVICE_ENUMERATOR: GUID = guid(0xBCDE0395_E52F_467C_8E3D_C4579291692E);
/// `{A95664D2-9614-4F35-A746-DE8DB63617E6}` (mmdeviceapi.h).
const IID_IMMDEVICE_ENUMERATOR: GUID = guid(0xA95664D2_9614_4F35_A746_DE8DB63617E6);
/// `{1CB9AD4C-DBFA-4C32-B178-C2F568A703B2}` (audioclient.h).
const IID_IAUDIO_CLIENT: GUID = guid(0x1CB9AD4C_DBFA_4C32_B178_C2F568A703B2);
/// `{F294ACFC-3146-4483-A7BF-ADDCA7C260E2}` (audioclient.h).
const IID_IAUDIO_RENDER_CLIENT: GUID = guid(0xF294ACFC_3146_4483_A7BF_ADDCA7C260E2);
/// `{00000003-0000-0010-8000-00AA00389B71}` (ksmedia.h).
const SUBTYPE_IEEE_FLOAT: GUID = guid(0x00000003_0000_0010_8000_00AA00389B71);
/// `{00000001-0000-0010-8000-00AA00389B71}` (ksmedia.h).
const SUBTYPE_PCM: GUID = guid(0x00000001_0000_0010_8000_00AA00389B71);
/// `PKEY_Device_FriendlyName` (propkey.h): fmtid `{A45C254E-DF1C-4EFD-8020-67D146A850E0}`, pid 14.
const PKEY_DEVICE_FRIENDLY_NAME: PROPERTYKEY =
    PROPERTYKEY { fmtid: guid(0xA45C254E_DF1C_4EFD_8020_67D146A850E0), pid: 14 };

/// Field-wise GUID equality (windows-sys's GUID does not derive PartialEq).
const fn guid_eq(a: &GUID, b: &GUID) -> bool {
    a.data1 == b.data1
        && a.data2 == b.data2
        && a.data3 == b.data3
        && a.data4[0] == b.data4[0]
        && a.data4[1] == b.data4[1]
        && a.data4[2] == b.data4[2]
        && a.data4[3] == b.data4[3]
        && a.data4[4] == b.data4[4]
        && a.data4[5] == b.data4[5]
        && a.data4[6] == b.data4[6]
        && a.data4[7] == b.data4[7]
}

/// `WAVE_FORMAT_PCM` = 1 (mmreg.h).
const TAG_PCM: u16 = 1;
/// `WAVE_FORMAT_IEEE_FLOAT` = 3 (mmreg.h).
const TAG_IEEE_FLOAT: u16 = 3;
/// `WAVE_FORMAT_EXTENSIBLE` = 0xFFFE (mmreg.h).
const TAG_EXTENSIBLE: u16 = 0xFFFE;
/// `KSAUDIO_SPEAKER_MONO` = `SPEAKER_FRONT_CENTER` (ksmedia.h).
const MASK_MONO: u32 = 0x4;
/// `KSAUDIO_SPEAKER_STEREO` = `SPEAKER_FRONT_LEFT | SPEAKER_FRONT_RIGHT` (ksmedia.h).
const MASK_STEREO: u32 = 0x3;

/// Rates probed for capability reports and the open ladder, ordered by likelihood (the Phase A
/// lesson: order by what usually works, not by what we prefer — 44.1/48 first because
/// redirection endpoints and consumer interfaces live there).
const PROBE_RATES: [u32; 8] = [44_100, 48_000, 96_000, 88_200, 192_000, 176_400, 384_000, 352_800];

/// Channel counts probed for the capability envelope (plan §9.6 ceiling: 64; we probe to 32 —
/// each probe costs one `IsFormatSupported`).
const PROBE_CHANNELS: [u16; 6] = [2, 1, 4, 8, 16, 32];

// --------------------------------------------------------------------------- COM vtables
//
// One #[repr(C)] struct pair per interface: an object (a single vtable pointer) and its vtable.
// Slot order is the SDK header declaration order, which IS the ABI — it has not changed since
// Windows Vista and cannot change without breaking every binary on Windows. The first three
// slots of every interface are IUnknown's (QueryInterface, AddRef, Release); `Com<T>::drop`
// relies on that prefix being layout-identical across all interfaces here.
//
// SAFETY contract for every fn-pointer field below: these are COM stdcall methods; `this` must
// be a live, owned interface pointer of the matching type, and all out-pointers must be valid
// for the duration of the call. The `Com<T>` wrappers are the only callers and uphold this.

/// Shape of the parameterless COM methods that return an HRESULT (`Start`, `Stop`, `Reset`,
/// `Commit`). SAFETY: `this` must be a live, owned interface pointer of the matching
/// type — every caller goes through the `Com<T>` wrappers below, which guarantee exactly that.
type HrFn<T> = unsafe extern "system" fn(this: *mut T) -> HRESULT;

/// Any COM interface, seen only through its IUnknown prefix (used by `Com<T>::drop`).
///
/// SAFETY contract for every fn-pointer field in every vtable below: these are COM stdcall
/// methods; `this` must be a live, owned interface pointer of the matching type, and all
/// out-pointers must be valid for the duration of the call. The `Com<T>` wrappers are the only
/// callers and uphold this. Slot order is the SDK header declaration order, which IS the ABI.
#[repr(C)]
struct UnknownVtbl {
    // SAFETY: IUnknown prefix (slots 0-2). `this` is a live, owned interface pointer (the
    // Com<T> invariant); Release is invoked exactly once, by Com<T>::drop.
    QueryInterface: unsafe extern "system" fn(
        this: *mut c_void,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT,
    AddRef: unsafe extern "system" fn(this: *mut c_void) -> u32,
    Release: unsafe extern "system" fn(this: *mut c_void) -> u32,
}

#[repr(C)]
struct AnyCom {
    lpVtbl: *const UnknownVtbl,
}

/// IMMDeviceEnumerator (mmdeviceapi.h). Slots 0–2 are IUnknown's (see `UnknownVtbl`'s contract);
/// slots 3–7 follow the header declaration order exactly.
#[repr(C)]
struct MmDeviceEnumerator {
    lpVtbl: *const MmDeviceEnumeratorVtbl,
}
#[repr(C)]
struct MmDeviceEnumeratorVtbl {
    // SAFETY: IUnknown prefix (slots 0-2). `this` is a live, owned interface pointer (the
    // Com<T> invariant); Release is invoked exactly once, by Com<T>::drop.
    QueryInterface: unsafe extern "system" fn(
        this: *mut MmDeviceEnumerator,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT,
    AddRef: unsafe extern "system" fn(this: *mut MmDeviceEnumerator) -> u32,
    Release: unsafe extern "system" fn(this: *mut MmDeviceEnumerator) -> u32,
    // SAFETY: slot 3 — EnumAudioEndpoints; out pointer receives a +1-referenced collection the
    // caller must own (Com<T>) or release.
    EnumAudioEndpoints: unsafe extern "system" fn(
        this: *mut MmDeviceEnumerator,
        flow: EDataFlow,
        state_mask: u32,
        out: *mut *mut MmDeviceCollection,
    ) -> HRESULT,
    GetDefaultAudioEndpoint: unsafe extern "system" fn(
        this: *mut MmDeviceEnumerator,
        flow: EDataFlow,
        role: ERole,
        out: *mut *mut MmDevice,
    ) -> HRESULT,
    // SAFETY: slot 5 — GetDevice's `id` is a null-terminated UTF-16 endpoint id (from GetId);
    // the out pointer receives a +1 reference the caller owns via Com<T>.
    GetDevice: unsafe extern "system" fn(
        this: *mut MmDeviceEnumerator,
        id: PCWSTR,
        out: *mut *mut MmDevice,
    ) -> HRESULT,
    // The notification-client parameter is typed as opaque: sparq never registers callbacks in
    // Phase 0 (polling enumeration at open time is the honest, allocation-free choice), but the
    // SAFETY: slots 6–7 keep positions correct; never called by sparq.
    RegisterEndpointNotificationCallback:
        unsafe extern "system" fn(this: *mut MmDeviceEnumerator, client: *mut c_void) -> HRESULT,
    UnregisterEndpointNotificationCallback:
        unsafe extern "system" fn(this: *mut MmDeviceEnumerator, client: *mut c_void) -> HRESULT,
}

/// IMMDeviceCollection (mmdeviceapi.h): slots 3–4.
#[repr(C)]
struct MmDeviceCollection {
    lpVtbl: *const MmDeviceCollectionVtbl,
}
#[repr(C)]
struct MmDeviceCollectionVtbl {
    // SAFETY: IUnknown prefix (slots 0-2). `this` is a live, owned interface pointer (the
    // Com<T> invariant); Release is invoked exactly once, by Com<T>::drop.
    QueryInterface: unsafe extern "system" fn(
        this: *mut MmDeviceCollection,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT,
    AddRef: unsafe extern "system" fn(this: *mut MmDeviceCollection) -> u32,
    Release: unsafe extern "system" fn(this: *mut MmDeviceCollection) -> u32,
    // SAFETY: slot 4 — GetCount writes a local; slot 5 — Item returns a +1 reference.
    GetCount: unsafe extern "system" fn(this: *mut MmDeviceCollection, out: *mut u32) -> HRESULT,
    Item: unsafe extern "system" fn(
        this: *mut MmDeviceCollection,
        index: u32,
        out: *mut *mut MmDevice,
    ) -> HRESULT,
}

/// IMMDevice (mmdeviceapi.h): slots 3–6.
#[repr(C)]
struct MmDevice {
    lpVtbl: *const MmDeviceVtbl,
}
#[repr(C)]
struct MmDeviceVtbl {
    // SAFETY: IUnknown prefix (slots 0-2). `this` is a live, owned interface pointer (the
    // Com<T> invariant); Release is invoked exactly once, by Com<T>::drop.
    QueryInterface: unsafe extern "system" fn(
        this: *mut MmDevice,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT,
    AddRef: unsafe extern "system" fn(this: *mut MmDevice) -> u32,
    Release: unsafe extern "system" fn(this: *mut MmDevice) -> u32,
    // SAFETY: slot 3 — Activate's out pointer receives a +1 reference of the requested IID.
    Activate: unsafe extern "system" fn(
        this: *mut MmDevice,
        iid: *const GUID,
        cls_ctx: u32,
        activation_params: *const PROPVARIANT,
        out: *mut *mut c_void,
    ) -> HRESULT,
    OpenPropertyStore: unsafe extern "system" fn(
        this: *mut MmDevice,
        access: u32,
        out: *mut *mut PropertyStore,
    ) -> HRESULT,
    // SAFETY: slot 5 — GetId's out pointer receives a CoTaskMem-allocated, null-terminated
    // UTF-16 string; the wrapper copies it and frees with CoTaskMemFree on every path.
    GetId: unsafe extern "system" fn(this: *mut MmDevice, out: *mut PWSTR) -> HRESULT,
    GetState: unsafe extern "system" fn(this: *mut MmDevice, out: *mut u32) -> HRESULT,
}

/// IPropertyStore (propsys.h): slots 3–7. Only GetValue is used; the rest hold positions.
#[repr(C)]
struct PropertyStore {
    lpVtbl: *const PropertyStoreVtbl,
}
#[repr(C)]
struct PropertyStoreVtbl {
    // SAFETY: IUnknown prefix (slots 0-2). `this` is a live, owned interface pointer (the
    // Com<T> invariant); Release is invoked exactly once, by Com<T>::drop.
    QueryInterface: unsafe extern "system" fn(
        this: *mut PropertyStore,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT,
    AddRef: unsafe extern "system" fn(this: *mut PropertyStore) -> u32,
    Release: unsafe extern "system" fn(this: *mut PropertyStore) -> u32,
    // SAFETY: slots 3–5 (Commit, GetCount, GetAt) keep positions correct; never called.
    Commit: HrFn<PropertyStore>,
    GetCount: unsafe extern "system" fn(this: *mut PropertyStore, out: *mut u32) -> HRESULT,
    GetAt: unsafe extern "system" fn(
        this: *mut PropertyStore,
        index: u32,
        out: *mut PROPERTYKEY,
    ) -> HRESULT,
    // SAFETY: slot 6 — GetValue writes a PROPVARIANT the caller must clear with PropVariantClear
    // (the wrapper does, on every path).
    GetValue: unsafe extern "system" fn(
        this: *mut PropertyStore,
        key: *const PROPERTYKEY,
        out: *mut PROPVARIANT,
    ) -> HRESULT,
    SetValue: unsafe extern "system" fn(
        this: *mut PropertyStore,
        key: *const PROPERTYKEY,
        value: *const PROPVARIANT,
    ) -> HRESULT,
}

/// IAudioClient (audioclient.h): slots 3–14, in header order.
#[repr(C)]
struct AudioClient {
    lpVtbl: *const AudioClientVtbl,
}
#[repr(C)]
struct AudioClientVtbl {
    // SAFETY: IUnknown prefix (slots 0-2). `this` is a live, owned interface pointer (the
    // Com<T> invariant); Release is invoked exactly once, by Com<T>::drop.
    QueryInterface: unsafe extern "system" fn(
        this: *mut AudioClient,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT,
    AddRef: unsafe extern "system" fn(this: *mut AudioClient) -> u32,
    Release: unsafe extern "system" fn(this: *mut AudioClient) -> u32,
    // SAFETY: slots 3–4. Format pointers reference caller locals that outlive the call;
    // Initialize's durations are in 100 ns units (REFERENCE_TIME).
    Initialize: unsafe extern "system" fn(
        this: *mut AudioClient,
        share_mode: i32,
        stream_flags: u32,
        buffer_duration_100ns: i64,
        periodicity_100ns: i64,
        format: *const WAVEFORMATEX,
        session_guid: *const GUID,
    ) -> HRESULT,
    GetBufferSize:
        unsafe extern "system" fn(this: *mut AudioClient, out_frames: *mut u32) -> HRESULT,
    // SAFETY: slots 5–7. GetStreamLatency is never called; GetCurrentPadding's out param is a
    // caller local; errors include DEVICE_INVALIDATED (the wrapper classifies).
    GetStreamLatency:
        unsafe extern "system" fn(this: *mut AudioClient, out_100ns: *mut i64) -> HRESULT,
    GetCurrentPadding:
        unsafe extern "system" fn(this: *mut AudioClient, out_frames: *mut u32) -> HRESULT,
    IsFormatSupported: unsafe extern "system" fn(
        this: *mut AudioClient,
        share_mode: i32,
        format: *const WAVEFORMATEX,
        closest_match: *mut *mut WAVEFORMATEX,
    ) -> HRESULT,
    // SAFETY: slots 8–9. GetMixFormat's out pointer receives a CoTaskMem allocation the wrapper
    // copies and frees; ppClosestMatch (S_FALSE path) is freed unread.
    GetMixFormat:
        unsafe extern "system" fn(this: *mut AudioClient, out: *mut *mut WAVEFORMATEX) -> HRESULT,
    GetDevicePeriod: unsafe extern "system" fn(
        this: *mut AudioClient,
        default_100ns: *mut i64,
        min_100ns: *mut i64,
    ) -> HRESULT,
    // SAFETY: slots 10–14 (Start, Stop, Reset, SetEventHandle, GetService). SetEventHandle takes
    // an owned kernel event handle (the stream keeps ownership, closing it after joining the
    // pump); GetService's out pointer receives a +1 reference the caller must own.
    Start: HrFn<AudioClient>,
    Stop: HrFn<AudioClient>,
    Reset: HrFn<AudioClient>,
    SetEventHandle: unsafe extern "system" fn(this: *mut AudioClient, event: HANDLE) -> HRESULT,
    GetService: unsafe extern "system" fn(
        this: *mut AudioClient,
        iid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT,
}

/// IAudioRenderClient (audioclient.h): slots 3–4.
#[repr(C)]
struct AudioRenderClient {
    lpVtbl: *const AudioRenderClientVtbl,
}
#[repr(C)]
struct AudioRenderClientVtbl {
    // SAFETY: IUnknown prefix (slots 0-2). `this` is a live, owned interface pointer (the
    // Com<T> invariant); Release is invoked exactly once, by Com<T>::drop.
    QueryInterface: unsafe extern "system" fn(
        this: *mut AudioRenderClient,
        riid: *const GUID,
        out: *mut *mut c_void,
    ) -> HRESULT,
    AddRef: unsafe extern "system" fn(this: *mut AudioRenderClient) -> u32,
    Release: unsafe extern "system" fn(this: *mut AudioRenderClient) -> u32,
    // SAFETY: slot 3 — GetBuffer's out pointer receives a device-owned buffer valid for exactly
    // the requested frames until ReleaseBuffer (the allowlist's central invariant; the wrapper
    // never lets the pointer escape). Slot 4 — ReleaseBuffer must pair every successful GetBuffer.
    GetBuffer: unsafe extern "system" fn(
        this: *mut AudioRenderClient,
        frames: u32,
        out: *mut *mut u8,
    ) -> HRESULT,
    ReleaseBuffer: unsafe extern "system" fn(
        this: *mut AudioRenderClient,
        frames_written: u32,
        flags: u32,
    ) -> HRESULT,
}

// NOTE: IAudioClock is deliberately NOT bound. Defect #76 decoded its `GetPosition` on the
// SATURN endpoints: it advances in buffer-sized steps per event tick — `(buffer_frames ÷
// event_period) ÷ rate − 1` reproduced the measured +1.2 M ppm "drift" to four digits on BOTH
// the RDP and the Behringer sessions (docs/hal/windows-notes.md §4b). It is not a clock, and
// binding it would tempt somebody to trust it again. The drift metric counts frames the device
// ACCEPTED against the wall instead — the pump knows that number exactly, on every backend.

// --------------------------------------------------------------------------- owned pointers

/// An owned COM interface pointer: the single owner, `Release`d exactly once in `Drop`.
///
/// The typed method wrappers below are the only sanctioned way to call through it; each carries
/// its own SAFETY text at the vtable call.
struct Com<T> {
    ptr: *mut T,
}

// SAFETY: WASAPI interfaces here are created inside MTA and used only by threads that are
// themselves MTA-initialised (`ensure_control_com` for control-side calls, `PumpCom` for the
// pump). MTA pointers are valid process-wide without marshalling, and `Com` ownership is moved
// (never cloned) between threads — `Send` expresses exactly that transfer, and nothing else in
// the module hands the pointer to a second thread while one holds it.
unsafe impl<T> Send for Com<T> {}

impl<T> Drop for Com<T> {
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }
        // SAFETY: every interface struct in this module is #[repr(C)] with the vtable pointer as
        // its first field, and every vtable begins with IUnknown's three slots (the COM ABI).
        // Reinterpreting `*mut T` as `*mut AnyCom` therefore reads the same memory with the same
        // layout, and Release is invoked exactly once for this owned pointer (Com is never
        // cloned). After the call the pointer is nulled so a second drop cannot re-release.
        unsafe {
            let any = self.ptr as *mut AnyCom;
            let release = (*(*any).lpVtbl).Release;
            release(any as *mut c_void);
        }
        self.ptr = ptr::null_mut();
    }
}

/// Take ownership of a +1-referenced out pointer after a vtable call.
///
/// SAFETY: `hr` is the call's HRESULT; on success `out` holds a +1 reference that
/// this function converts into sole ownership (released by `Com::drop`). On failure `out` is
/// either null or unreferenced by contract, and nothing is owned.
unsafe fn com_own<T>(hr: HRESULT, out: *mut T, context: &str) -> Result<Com<T>, HalError> {
    if hr < 0 {
        return Err(classify(context, hr));
    }
    if out.is_null() {
        return Err(HalError::System(format!("{context}: S_OK with a null interface pointer")));
    }
    Ok(Com { ptr: out })
}

// ---- typed wrappers: the only places vtable slots are called ------------------------------

impl Com<MmDeviceEnumerator> {
    /// Slot 4 — `IMMDeviceEnumerator::EnumAudioEndpoints`.
    fn enum_endpoints(
        &self,
        flow: EDataFlow,
        mask: u32,
    ) -> Result<Com<MmDeviceCollection>, HalError> {
        let mut out: *mut MmDeviceCollection = ptr::null_mut();
        // SAFETY: `self` is a live owned interface on an MTA thread; `out` is a local that
        // receives a +1 reference on success, owned via `com_own`.
        let hr =
            unsafe { ((*(*self.ptr).lpVtbl).EnumAudioEndpoints)(self.ptr, flow, mask, &mut out) };
        unsafe { com_own(hr, out, "EnumAudioEndpoints") }
    }

    /// Slot 5 — `IMMDeviceEnumerator::GetDefaultAudioEndpoint`.
    fn default_endpoint(&self, flow: EDataFlow, role: ERole) -> Result<Com<MmDevice>, HalError> {
        let mut out: *mut MmDevice = ptr::null_mut();
        // SAFETY: as `enum_endpoints`. A machine with no default render endpoint returns
        // E_NOT_FOUND, which callers treat as "no default", not as failure.
        let hr = unsafe {
            ((*(*self.ptr).lpVtbl).GetDefaultAudioEndpoint)(self.ptr, flow, role, &mut out)
        };
        unsafe { com_own(hr, out, "GetDefaultAudioEndpoint") }
    }
}

impl Com<MmDeviceCollection> {
    /// Slot 4 — `IMMDeviceCollection::GetCount`.
    fn count(&self) -> Result<u32, HalError> {
        let mut n = 0u32;
        // SAFETY: live owned interface; out param is a local.
        let hr = unsafe { ((*(*self.ptr).lpVtbl).GetCount)(self.ptr, &mut n) };
        if hr < 0 {
            return Err(classify("IMMDeviceCollection::GetCount", hr));
        }
        Ok(n)
    }

    /// Slot 5 — `IMMDeviceCollection::Item`.
    fn item(&self, index: u32) -> Result<Com<MmDevice>, HalError> {
        let mut out: *mut MmDevice = ptr::null_mut();
        // SAFETY: live owned interface; `index` was bounds-checked against GetCount by the
        // caller; `out` receives a +1 reference on success.
        let hr = unsafe { ((*(*self.ptr).lpVtbl).Item)(self.ptr, index, &mut out) };
        unsafe { com_own(hr, out, "IMMDeviceCollection::Item") }
    }
}

impl Com<MmDevice> {
    /// Slot 4 — `IMMDevice::Activate` for `IAudioClient`.
    fn activate_client(&self) -> Result<Com<AudioClient>, HalError> {
        let mut out: *mut c_void = ptr::null_mut();
        // SAFETY: live owned interface; null activation params (documented for IAudioClient);
        // CLSCTX_ALL; `out` receives a +1 reference on success, re-typed to the requested IID.
        let hr = unsafe {
            ((*(*self.ptr).lpVtbl).Activate)(
                self.ptr,
                &IID_IAUDIO_CLIENT,
                CLSCTX_ALL,
                ptr::null(),
                &mut out,
            )
        };
        unsafe { com_own(hr, out as *mut AudioClient, "IMMDevice::Activate(IAudioClient)") }
    }

    /// Slot 5 — `IMMDevice::OpenPropertyStore` (read-only).
    fn property_store(&self) -> Result<Com<PropertyStore>, HalError> {
        let mut out: *mut PropertyStore = ptr::null_mut();
        // SAFETY: live owned interface; STGM_READ; `out` receives a +1 reference on success.
        let hr =
            unsafe { ((*(*self.ptr).lpVtbl).OpenPropertyStore)(self.ptr, STGM_READ, &mut out) };
        unsafe { com_own(hr, out, "IMMDevice::OpenPropertyStore") }
    }

    /// Slot 6 — `IMMDevice::GetId`; the id string is copied out and the CoTaskMem allocation
    /// freed here, so nothing raw escapes.
    fn endpoint_id(&self) -> Result<String, HalError> {
        let mut raw: PWSTR = ptr::null_mut();
        // SAFETY: live owned interface; `raw` receives a CoTaskMem-allocated, null-terminated
        // UTF-16 string that is copied into a String and freed with CoTaskMemFree on every path.
        let hr = unsafe { ((*(*self.ptr).lpVtbl).GetId)(self.ptr, &mut raw) };
        if hr < 0 {
            return Err(classify("IMMDevice::GetId", hr));
        }
        let s = unsafe { pwstr_to_string(raw) }
            .ok_or_else(|| HalError::System(String::from("IMMDevice::GetId returned a null id")));
        // SAFETY: `raw` is the CoTaskMem allocation from GetId, freed exactly once here.
        unsafe { CoTaskMemFree(raw as *const c_void) };
        s
    }

    /// The friendly name via the property store, if present.
    fn friendly_name(&self) -> Option<String> {
        let store = match self.property_store() {
            Ok(s) => s,
            Err(e) => {
                note_name_failure(&format!("OpenPropertyStore: {e}"));
                return None;
            },
        };
        // SAFETY: zeroed PROPVARIANT is the documented "empty" input; GetValue fills it and
        // PropVariantClear releases whatever it allocated, on every path. The union field is
        // read only after checking the type tag is VT_LPWSTR (the documented type for this key);
        // `vt` is copied out before the clear so the failure note cannot read freed memory.
        unsafe {
            let mut pv: PROPVARIANT = core::mem::zeroed();
            let hr =
                ((*(*store.ptr).lpVtbl).GetValue)(store.ptr, &PKEY_DEVICE_FRIENDLY_NAME, &mut pv);
            let vt = pv.Anonymous.Anonymous.vt;
            let name = if hr >= 0 && vt == VT_LPWSTR {
                pwstr_to_string(pv.Anonymous.Anonymous.Anonymous.pwszVal)
            } else {
                None
            };
            let _ = PropVariantClear(&mut pv);
            if name.is_none() {
                note_name_failure(&format!("GetValue HRESULT {hr:#010x}, vt {vt}"));
            }
            name
        }
    }
}

impl Com<AudioClient> {
    /// Slot 4 — `IAudioClient::Initialize`.
    fn initialize(
        &self,
        share: i32,
        flags: u32,
        duration_100ns: i64,
        periodicity_100ns: i64,
        format: &WAVEFORMATEX,
    ) -> HRESULT {
        // SAFETY: live owned client; `format` points to a caller local that outlives the call;
        // null session GUID (documented default). The HRESULT is returned raw because the
        // alignment two-step needs to see NOT_ALIGNED specifically.
        unsafe {
            ((*(*self.ptr).lpVtbl).Initialize)(
                self.ptr,
                share,
                flags,
                duration_100ns,
                periodicity_100ns,
                format,
                ptr::null(),
            )
        }
    }

    /// Slot 5 — `IAudioClient::GetBufferSize` (the device's true period, in frames).
    fn buffer_size(&self) -> Result<u32, HalError> {
        let mut frames = 0u32;
        // SAFETY: live owned client; out param is a local.
        let hr = unsafe { ((*(*self.ptr).lpVtbl).GetBufferSize)(self.ptr, &mut frames) };
        if hr < 0 {
            return Err(classify("IAudioClient::GetBufferSize", hr));
        }
        Ok(frames)
    }

    /// Slot 7 — `IAudioClient::GetCurrentPadding`.
    fn current_padding(&self) -> Result<u32, HalError> {
        let mut frames = 0u32;
        // SAFETY: live owned client; out param is a local. Errors include DEVICE_INVALIDATED,
        // which the pump maps to the Removed state.
        let hr = unsafe { ((*(*self.ptr).lpVtbl).GetCurrentPadding)(self.ptr, &mut frames) };
        if hr < 0 {
            return Err(classify("IAudioClient::GetCurrentPadding", hr));
        }
        Ok(frames)
    }

    /// Slot 8 — `IAudioClient::IsFormatSupported`, disambiguated: S_OK = exactly supported,
    /// S_FALSE = not supported (closest match returned and freed), negative = hard refusal.
    fn probe_format(&self, share: i32, format: &WAVEFORMATEX) -> HRESULT {
        let mut closest: *mut WAVEFORMATEX = ptr::null_mut();
        // SAFETY: live owned client; `format` outlives the call; `closest` receives a CoTaskMem
        // allocation only on S_FALSE and is freed immediately without being read (we asked a
        // yes/no question; guessing from "closest" is how latency lies are born).
        let hr = unsafe {
            ((*(*self.ptr).lpVtbl).IsFormatSupported)(self.ptr, share, format, &mut closest)
        };
        if !closest.is_null() {
            // SAFETY: documented CoTaskMem allocation; freed exactly once; not read.
            unsafe { CoTaskMemFree(closest as *const c_void) };
        }
        hr
    }

    fn format_supported(&self, share: i32, format: &WAVEFORMATEX) -> bool {
        self.probe_format(share, format) == 0 // S_OK only; S_FALSE = "no, closest match set"
    }

    /// Probe with the **extensible** form of the same format.
    ///
    /// Not a redundancy: the SATURN/RDP endpoint refused every plain `WAVEFORMATEX` f32 probe
    /// with `AUDCLNT_E_UNSUPPORTED_FORMAT` while accepting its own `WAVEFORMATEXTENSIBLE` mix
    /// format verbatim (defect #46). Probing both forms is the difference between "this device
    /// supports 44.1 kHz" as a measurement and as a guess.
    fn format_supported_ext(&self, share: i32, rate: u32, ch: u16, mask: u32) -> bool {
        let wfxe = wfxe_f32(rate, ch, mask);
        self.probe_format(share, &wfxe.Format) == 0
    }

    /// Slot 9 — `IAudioClient::GetMixFormat`, returned as copies (the CoTaskMem original is
    /// freed inside, so no raw pointer escapes).
    fn mix_format(&self) -> Result<(WAVEFORMATEX, Option<WAVEFORMATEXTENSIBLE>), HalError> {
        let mut raw: *mut WAVEFORMATEX = ptr::null_mut();
        // SAFETY: live owned client; `raw` receives a CoTaskMem allocation, read and freed here.
        let hr = unsafe { ((*(*self.ptr).lpVtbl).GetMixFormat)(self.ptr, &mut raw) };
        if hr < 0 || raw.is_null() {
            return Err(classify("IAudioClient::GetMixFormat", hr));
        }
        // SAFETY: GetMixFormat returns at least a WAVEFORMATEX; when its tag says EXTENSIBLE the
        // allocation is a WAVEFORMATEXTENSIBLE (documented layout contract) — cast is guarded by
        // the tag check. Both structs are Copy; freed before return.
        let (base, ext) = unsafe {
            let base = *raw;
            let ext = if base.wFormatTag == TAG_EXTENSIBLE {
                Some(*(raw as *const WAVEFORMATEXTENSIBLE))
            } else {
                None
            };
            CoTaskMemFree(raw as *const c_void);
            (base, ext)
        };
        Ok((base, ext))
    }

    /// Slot 10 — `IAudioClient::GetDevicePeriod` (100 ns units).
    fn device_period(&self) -> (Option<Duration>, Option<Duration>) {
        let (mut def, mut min) = (0i64, 0i64);
        // SAFETY: live owned client; out params are locals; failure yields (None, None) rather
        // than a guessed number.
        let hr = unsafe { ((*(*self.ptr).lpVtbl).GetDevicePeriod)(self.ptr, &mut def, &mut min) };
        if hr < 0 {
            return (None, None);
        }
        (
            (def > 0).then(|| Duration::from_nanos(def as u64 * 100)),
            (min > 0).then(|| Duration::from_nanos(min as u64 * 100)),
        )
    }

    /// Slot 11 — `IAudioClient::Start`.
    fn start(&self) -> Result<(), HalError> {
        // SAFETY: live owned, initialised client; called only from the pump thread that owns it.
        let hr = unsafe { ((*(*self.ptr).lpVtbl).Start)(self.ptr) };
        if hr < 0 {
            return Err(classify("IAudioClient::Start", hr));
        }
        Ok(())
    }

    /// Slot 12 — `IAudioClient::Stop`.
    fn stop(&self) {
        // SAFETY: live owned client; Stop is safe to call on a started-or-not stream; the result
        // is deliberately dropped (teardown path — nothing better to do than log elsewhere).
        unsafe {
            let _ = ((*(*self.ptr).lpVtbl).Stop)(self.ptr);
        }
    }

    /// Slot 14 — `IAudioClient::SetEventHandle`.
    fn set_event_handle(&self, event: HANDLE) -> Result<(), HalError> {
        // SAFETY: live owned client; `event` is a valid kernel handle owned by the stream (it
        // outlives the client: closed in the stream's Drop after joining the pump).
        let hr = unsafe { ((*(*self.ptr).lpVtbl).SetEventHandle)(self.ptr, event) };
        if hr < 0 {
            return Err(classify("IAudioClient::SetEventHandle", hr));
        }
        Ok(())
    }

    /// Slot 15 — `IAudioClient::GetService` for `IAudioRenderClient`.
    fn render_client(&self) -> Result<Com<AudioRenderClient>, HalError> {
        let mut out: *mut c_void = ptr::null_mut();
        // SAFETY: live owned, initialised client; `out` receives a +1 reference on success.
        let hr = unsafe {
            ((*(*self.ptr).lpVtbl).GetService)(self.ptr, &IID_IAUDIO_RENDER_CLIENT, &mut out)
        };
        unsafe { com_own(hr, out as *mut AudioRenderClient, "GetService(IAudioRenderClient)") }
    }

    // NOTE: no `GetService(IAudioClock)` binding — see the note where its vtable used to be
    // (defect #76: its GetPosition advances in buffer steps per event tick; it is not a clock).
}

impl Com<AudioRenderClient> {
    /// The only sanctioned device-write path: `GetBuffer(frames)` → fill from the FIFO in the
    /// negotiated format → `ReleaseBuffer(frames)`. Returns how many samples came *from the
    /// FIFO* (a shortfall means the tail was silence: an xrun the caller must count).
    ///
    /// The device pointer never escapes this function — the allowlist invariant ("valid for
    /// exactly `frame_count` frames and only until ReleaseBuffer") is enforced by construction,
    /// not by discipline at twenty call sites.
    fn write_from_fifo(
        &self,
        frames: u32,
        channels: usize,
        fmt: DevFmt,
        fifo: &mut Fifo,
    ) -> Result<usize, HalError> {
        let mut data: *mut u8 = ptr::null_mut();
        // SAFETY: live owned client; `frames` ≤ the device period (callers clamp to
        // period − padding); `data` is a local receiving the device-owned buffer pointer.
        let hr = unsafe { ((*(*self.ptr).lpVtbl).GetBuffer)(self.ptr, frames, &mut data) };
        if hr < 0 {
            return Err(classify("IAudioRenderClient::GetBuffer", hr));
        }
        if data.is_null() {
            return Err(HalError::Device(String::from(
                "GetBuffer: S_OK with a null data pointer (driver bug — report this device)",
            )));
        }
        // SAFETY: `data` is valid for exactly `frames * channels` samples of the negotiated
        // format (recorded at open, carried by `fmt`), until the ReleaseBuffer below; the
        // slices are built to that exact length; the f32/i16/i32 casts match the format the
        // ladder negotiated; the FIFO write does not alias the device buffer.
        let got = unsafe {
            match fmt {
                DevFmt::F32 => {
                    let out = std::slice::from_raw_parts_mut(
                        data as *mut f32,
                        frames as usize * channels,
                    );
                    fifo.pop_into_f32(out)
                },
                DevFmt::I16 => {
                    let out = std::slice::from_raw_parts_mut(
                        data as *mut i16,
                        frames as usize * channels,
                    );
                    fifo.pop_into_i16(out)
                },
                DevFmt::I32 => {
                    let out = std::slice::from_raw_parts_mut(
                        data as *mut i32,
                        frames as usize * channels,
                    );
                    fifo.pop_into_i32(out)
                },
            }
        };
        // SAFETY: `frames` written ≤ the GetBuffer request (the buffer was filled completely —
        // silence where the FIFO ran short); exactly one ReleaseBuffer per GetBuffer on every
        // path (the early returns above happen before GetBuffer succeeded).
        let hr = unsafe { ((*(*self.ptr).lpVtbl).ReleaseBuffer)(self.ptr, frames, 0) };
        if hr < 0 {
            return Err(classify("IAudioRenderClient::ReleaseBuffer", hr));
        }
        Ok(got)
    }
}

/// PWSTR → String without taking ownership. Bounded scan: a driver that hands back a
/// non-terminated string must not be able to walk us off the heap.
///
/// SAFETY: `p` is null or a valid, null-terminated (or at least heap-bounded) UTF-16
/// string owned by the caller's COM allocator; this function only reads, up to the bound.
unsafe fn pwstr_to_string(p: PWSTR) -> Option<String> {
    if p.is_null() {
        return None;
    }
    const BOUND: usize = 4096; // endpoint ids are ~100 chars; names are bounded by the OS
    let mut len = 0usize;
    // SAFETY: `p` is readable per the fn contract; the scan stops at the terminator or BOUND,
    // both of which are within the allocation's readable range for OS-produced strings. A string
    // that hits BOUND is truncated (and visibly so) rather than followed forever.
    unsafe {
        while len < BOUND && *p.add(len) != 0 {
            len += 1;
        }
        if len == 0 {
            return Some(String::new());
        }
        Some(String::from_utf16_lossy(std::slice::from_raw_parts(p, len)))
    }
}

/// One-time note when the friendly name cannot be read (defect #40 instrumentation): every
/// endpoint prints `<endpoint N>` instead, and the log must say WHY at least once so the next
/// hardware run either confirms the cause or clears it.
fn note_name_failure(detail: &str) {
    use std::sync::atomic::AtomicBool;
    static REPORTED: AtomicBool = AtomicBool::new(false);
    if !REPORTED.swap(true, Ordering::Relaxed) {
        eprintln!(
            "sparq hal[wasapi]: friendly-name read failed ({detail}) — endpoints show as \
             `<endpoint N>`. Please send this line back; names are cosmetic but the failure \
             mode is not (defect #40, docs/hal/windows-notes.md)"
        );
    }
}

// --------------------------------------------------------------------------- COM lifetime

/// Persistent per-thread MTA initialisation for control-side calls.
///
/// Deliberately never uninitialised while the thread lives: WASAPI interfaces created under this
/// init stay valid exactly as long as it does, and streams outlive single function calls. The
/// cost is one COM apartment per thread that touches the HAL — the CLI has one.
fn ensure_control_com() -> Result<(), HalError> {
    thread_local! {
        static COM: Result<(), String> = {
            // SAFETY: CoInitializeEx with null reserved pointer initialises this thread for
            // multithreaded COM; S_OK and S_FALSE (already initialised) both mean success here.
            // The matching CoUninitialize is deliberately never called (see fn docs) — the
            // apartment must outlive every interface created inside it.
            let hr = unsafe { CoInitializeEx(ptr::null(), COINIT_MULTITHREADED as u32) };
            if hr >= 0 {
                Ok(())
            } else if hr == RPC_E_CHANGED_MODE {
                Err(String::from(
                    "this thread already initialised COM as single-threaded (STA); the WASAPI HAL \
                     needs MTA. Phase 0 hosts are console apps — if this appears inside a GUI \
                     host, the shell (WO-012) must not OleInitialize on the audio-control thread",
                ))
            } else {
                Err(format!("CoInitializeEx failed (HRESULT {hr:#010x})"))
            }
        };
    }
    COM.with(Result::clone).map_err(HalError::System)
}

/// Scoped MTA init for the pump thread (which does end, so it does uninitialise).
struct PumpCom;
impl PumpCom {
    fn new() -> Result<Self, HalError> {
        // SAFETY: as above; this thread's init is paired with CoUninitialize in Drop.
        let hr = unsafe { CoInitializeEx(ptr::null(), COINIT_MULTITHREADED as u32) };
        if hr >= 0 {
            Ok(Self)
        } else {
            Err(HalError::System(format!("pump thread CoInitializeEx failed (HRESULT {hr:#010x})")))
        }
    }
}
impl Drop for PumpCom {
    fn drop(&mut self) {
        // SAFETY: paired 1:1 with this thread's successful CoInitializeEx in `new`.
        unsafe { CoUninitialize() };
    }
}

// --------------------------------------------------------------------------- error mapping

/// Human name for the HRESULTs sparq classifies; the hex always rides along so logs stay
/// searchable even for codes this table has never seen.
fn hr_text(hr: HRESULT) -> String {
    let name = match hr {
        x if x == AUDCLNT_E_DEVICE_INVALIDATED => "AUDCLNT_E_DEVICE_INVALIDATED",
        x if x == AUDCLNT_E_DEVICE_IN_USE => "AUDCLNT_E_DEVICE_IN_USE",
        x if x == AUDCLNT_E_EXCLUSIVE_MODE_NOT_ALLOWED => "AUDCLNT_E_EXCLUSIVE_MODE_NOT_ALLOWED",
        x if x == AUDCLNT_E_UNSUPPORTED_FORMAT => "AUDCLNT_E_UNSUPPORTED_FORMAT",
        x if x == AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED => "AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED",
        x if x == AUDCLNT_E_INVALID_DEVICE_PERIOD => "AUDCLNT_E_INVALID_DEVICE_PERIOD",
        _ => "",
    };
    if name.is_empty() {
        format!("HRESULT {hr:#010x}")
    } else {
        format!("{name} ({hr:#010x})")
    }
}

/// Classify a WASAPI HRESULT into the [`HalError`] variant whose *recovery strategy* it belongs
/// to. One place, so the CLI and the logs always agree about what happened.
#[must_use]
pub fn classify(context: &str, hr: HRESULT) -> HalError {
    let text = format!("{context}: {}", hr_text(hr));
    if hr == AUDCLNT_E_DEVICE_INVALIDATED {
        HalError::Removed(text)
    } else if hr == AUDCLNT_E_EXCLUSIVE_MODE_NOT_ALLOWED || hr == AUDCLNT_E_DEVICE_IN_USE {
        HalError::Busy(text)
    } else if hr == AUDCLNT_E_UNSUPPORTED_FORMAT || hr == AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED {
        HalError::Format(text)
    } else {
        HalError::Device(text)
    }
}

// --------------------------------------------------------------------------- discovery types

/// One render endpoint as discovered, before it becomes a [`DeviceInfo`].
struct Discovered {
    endpoint_id: String,
    name: String,
}

/// The sample format of the device buffers, negotiated at open. The pump converts the FIFO's
/// f32 into this on every write. `I32` serves both the 32-bit and the 24-in-32 rungs: a
/// 24-in-32 sample is left-justified in its container (the 24-bit value in the top three
/// bytes), so the f32→i32 arithmetic is identical and the DAC ignores the low 8 bits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DevFmt {
    /// IEEE float — no conversion.
    F32,
    /// 16-bit PCM — conversion in the pump.
    I16,
    /// 32-bit container, 24 or 32 valid bits — conversion in the pump.
    I32,
}

/// The format a stream negotiated at `open` — everything the pump needs to interpret buffers.
#[derive(Clone, Copy, Debug)]
struct Negotiated {
    exclusive: bool,
    rate: u32,
    channels: u16,
    /// Device period in frames (from `GetBufferSize` — what the device actually does, not what
    /// we asked for).
    period_frames: u32,
    /// The period of the ask that opened, in frames — exclusive only (`None` in shared, where
    /// the engine picks its own buffer and an "ask" annotation would be noise). Printed next to
    /// `period_frames` when the two disagree: a driver that allocates less than it accepted is
    /// telling the truth through `GetBufferSize` and must not be hidden by the open line
    /// (defect #89's third face).
    ask_frames: Option<u32>,
    /// sparq block size in frames (what the callback sees). May differ from `period_frames`;
    /// the FIFO absorbs the mismatch.
    block_frames: u32,
    /// The device buffer sample format (the pump converts to it).
    fmt: DevFmt,
    /// What the negotiated format is called in logs, e.g. `i24-in-32 (converting)`.
    fmt_name: &'static str,
}

/// COM interfaces owned by a stream; ownership moves to the pump thread while it runs.
struct PumpBundle {
    client: Com<AudioClient>,
    render: Com<AudioRenderClient>,
}

/// Everything the pump thread owns while running, returned intact on exit (no leaks across
/// start/stop cycles).
struct PumpOwned {
    bundle: PumpBundle,
    proc: ProcessFn,
    fifo: Fifo,
    block: Vec<f32>,
}

/// State shared between the stream (control thread) and its pump.
struct PumpShared {
    stop: AtomicBool,
    state: AtomicU8,
    /// HRESULT of the last device-level failure (0 = none) — the pump writes, the stream reads.
    last_hr: AtomicI64,
    /// The bound event handle (as isize for atomic transport); `SetEvent` from `stop` wakes the
    /// pump immediately instead of at the next device period. 0 = none (closed in Drop).
    event: AtomicIsize,
}

// --------------------------------------------------------------------------- the FIFO

/// A fixed-capacity power-of-two FIFO of interleaved f32 samples. Single-threaded by contract
/// (the pump owns it while running); allocated and pre-touched at `open`, never on the audio path.
struct Fifo {
    buf: Vec<f32>,
    mask: usize,
    head: usize,
    tail: usize,
}

impl Fifo {
    fn new(capacity_samples: usize) -> Self {
        let slots = capacity_samples.max(64).next_power_of_two();
        let mut buf = vec![0.0f32; slots];
        // Pre-touch: fault every page in now, at open, on the control thread — a page fault
        // inside the callback is a syscall with unbounded latency (plan §5.2 rule 1).
        // SAFETY: f32 is a plain POD type; viewing the allocation as bytes preserves length
        // (len×4) and alignment (u8 aligns to 1). pre_touch writes back the bytes it reads, so
        // content is unchanged; only page residency changes.
        let bytes: &mut [u8] =
            unsafe { std::slice::from_raw_parts_mut(buf.as_mut_ptr().cast::<u8>(), buf.len() * 4) };
        pre_touch(bytes);
        Self { buf, mask: slots - 1, head: 0, tail: 0 }
    }

    #[inline]
    fn len(&self) -> usize {
        self.tail.wrapping_sub(self.head)
    }

    #[inline]
    fn free(&self) -> usize {
        self.buf.len() - self.len()
    }

    fn push(&mut self, src: &[f32]) {
        for &s in src {
            if self.free() == 0 {
                return; // overfill is a pump-logic bug; drop newest and let the xrun counter talk
            }
            self.buf[self.tail & self.mask] = s;
            self.tail = self.tail.wrapping_add(1);
        }
    }

    /// Pop into an f32 device buffer. Returns samples written from the FIFO; any remainder is
    /// zero-filled — silence is the honest sound of an xrun.
    fn pop_into_f32(&mut self, out: &mut [f32]) -> usize {
        let mut n = 0;
        while n < out.len() && self.len() > 0 {
            out[n] = self.buf[self.head & self.mask];
            self.head = self.head.wrapping_add(1);
            n += 1;
        }
        for s in out.iter_mut().skip(n) {
            *s = 0.0;
        }
        n
    }

    /// Pop into an i16 device buffer with clamping conversion (shared-mode 16-bit mix, or the
    /// exclusive i16 rung).
    fn pop_into_i16(&mut self, out: &mut [i16]) -> usize {
        let mut n = 0;
        while n < out.len() && self.len() > 0 {
            let s = self.buf[self.head & self.mask].clamp(-1.0, 1.0);
            // 32767 (not 32768) keeps -1.0 → -32767 symmetric and cannot overflow i16.
            out[n] = (s * 32767.0) as i16;
            self.head = self.head.wrapping_add(1);
            n += 1;
        }
        for s in out.iter_mut().skip(n) {
            *s = 0;
        }
        n
    }

    /// Pop into an i32 device buffer with clamping conversion (the exclusive integer rungs —
    /// 32-bit and 24-in-32 alike: the container is left-justified, so one arithmetic serves
    /// both and the DAC ignores the low 8 bits of a 24-in-32 stream).
    fn pop_into_i32(&mut self, out: &mut [i32]) -> usize {
        let mut n = 0;
        while n < out.len() && self.len() > 0 {
            let s = self.buf[self.head & self.mask].clamp(-1.0, 1.0);
            // 2^31 as f32: +1.0 lands exactly on 2^31, which Rust's saturating float→int cast
            // turns into i32::MAX — a one-LSB asymmetry against i32::MIN at full scale
            // (-180 dB, inaudible, and moot for 24-in-32 where the low 8 bits are padding).
            // -1.0 maps to i32::MIN exactly. The cast saturates, so nothing can wrap or trap.
            out[n] = (s * 2_147_483_648.0) as i32;
            self.head = self.head.wrapping_add(1);
            n += 1;
        }
        for s in out.iter_mut().skip(n) {
            *s = 0;
        }
        n
    }

    fn reset(&mut self) {
        self.head = 0;
        self.tail = 0;
    }
}

// --------------------------------------------------------------------------- the backend

/// WASAPI backend, one instance per share mode.
#[derive(Clone, Copy, Debug)]
pub struct WasapiBackend {
    kind: BackendKind,
}

impl WasapiBackend {
    /// The exclusive-mode backend (the stage path: lowest latency, no mixer, no other apps).
    #[must_use]
    pub const fn exclusive() -> Self {
        Self { kind: BackendKind::WasapiExclusive }
    }

    /// The shared-mode backend (everything else on Windows: the mixer stays in charge).
    #[must_use]
    pub const fn shared() -> Self {
        Self { kind: BackendKind::WasapiShared }
    }

    fn exclusive_mode(&self) -> bool {
        self.kind == BackendKind::WasapiExclusive
    }

    /// The raw enumerator, owned.
    fn enumerator() -> Result<Com<MmDeviceEnumerator>, HalError> {
        let mut out: *mut c_void = ptr::null_mut();
        // SAFETY: CoCreateInstance with the documented CLSID/IID pair, no aggregation, CLSCTX_ALL;
        // `out` receives a +1 reference on success, owned via `com_own`.
        let hr = unsafe {
            CoCreateInstance(
                &CLSID_MMDEVICE_ENUMERATOR,
                ptr::null_mut(),
                CLSCTX_ALL,
                &IID_IMMDEVICE_ENUMERATOR,
                &mut out,
            )
        };
        unsafe {
            com_own(hr, out as *mut MmDeviceEnumerator, "CoCreateInstance(MMDeviceEnumerator)")
        }
    }

    /// Every active render endpoint, default first.
    fn discover() -> Result<Vec<Discovered>, HalError> {
        ensure_control_com()?;
        let enumerator = Self::enumerator()?;
        let collection = enumerator.enum_endpoints(eRender, DEVICE_STATE_ACTIVE)?;
        let count = collection.count()?;

        // The default endpoint's id, for the "default first" ordering guarantee.
        let default_id = enumerator
            .default_endpoint(eRender, eMultimedia)
            .ok()
            .and_then(|d| d.endpoint_id().ok());

        let mut out: Vec<Discovered> = Vec::with_capacity(count as usize);
        for i in 0..count {
            let Ok(device) = collection.item(i) else { continue };
            let Ok(endpoint_id) = device.endpoint_id() else { continue };
            let name = device.friendly_name().unwrap_or_else(|| format!("<endpoint {i}>"));
            out.push(Discovered { endpoint_id, name });
        }
        if let Some(def) = default_id {
            if let Some(pos) = out.iter().position(|d| d.endpoint_id == def) {
                out.swap(0, pos); // default first: DeviceId::default_of must be truthful
            }
        }
        Ok(out)
    }

    /// Resolve a [`DeviceId`] to a live endpoint: by stable endpoint id first, index second
    /// (the Phase A lesson: a rig that re-enumerates after a USB hiccup must resolve identically).
    fn resolve(&self, dev: &DeviceId) -> Result<Com<MmDevice>, HalError> {
        if dev.backend != self.kind {
            return Err(HalError::NotFound(format!(
                "device id belongs to backend `{}`; this is `{}`",
                dev.backend, self.kind
            )));
        }
        let found = Self::discover()?;
        let want_id = match &dev.endpoint {
            Some(ep) => Some(ep.clone()),
            None => found
                .get(usize::try_from(dev.index).unwrap_or(usize::MAX))
                .map(|d| d.endpoint_id.clone()),
        };
        let Some(want_id) = want_id else {
            return Err(HalError::NotFound(format!(
                "no render endpoint for {:?} among {} device(s) — re-run `sparq devices`",
                dev,
                found.len()
            )));
        };
        let enumerator = Self::enumerator()?;
        let collection = enumerator.enum_endpoints(eRender, DEVICE_STATE_ACTIVE)?;
        let count = collection.count()?;
        for i in 0..count {
            let Ok(device) = collection.item(i) else { continue };
            if device.endpoint_id().ok().as_deref() == Some(want_id.as_str()) {
                return Ok(device);
            }
        }
        Err(HalError::NotFound(format!(
            "endpoint `{want_id}` vanished between discovery and open (replug?) — re-enumerate"
        )))
    }
}

impl HalBackend for WasapiBackend {
    fn kind(&self) -> BackendKind {
        self.kind
    }

    fn display_name(&self) -> &'static str {
        if self.exclusive_mode() {
            "WASAPI exclusive (event-driven)"
        } else {
            "WASAPI shared (event-driven)"
        }
    }

    fn enumerate(&self) -> Result<Vec<DeviceInfo>, HalError> {
        let found = Self::discover()?;
        let first_id = found.first().map(|d| d.endpoint_id.clone());
        Ok(found
            .into_iter()
            .enumerate()
            .map(|(i, d)| DeviceInfo {
                id: DeviceId {
                    backend: self.kind,
                    index: i as u32,
                    endpoint: Some(d.endpoint_id.clone()),
                },
                name: d.name,
                has_output: true,
                has_input: false, // increment 1 enumerates render endpoints only (module docs)
                is_default_output: first_id.as_deref() == Some(d.endpoint_id.as_str()),
                is_default_input: false,
            })
            .collect())
    }

    fn capabilities(&self, dev: &DeviceId) -> Result<Capabilities, HalError> {
        ensure_control_com()?;
        let device = self.resolve(dev)?;
        let client = device.activate_client()?;
        // WAVEFORMATEX(TENSIBLE) are packed(1) in windows-sys: referencing a packed field is
        // UB, so every value used below is copied out to a plain local immediately.
        let (mix, mix_ext) = client.mix_format()?;
        let mix_rate = mix.nSamplesPerSec;
        let mix_ch = mix.nChannels;
        let mix_tag = mix.wFormatTag;
        let mix_bits = mix.wBitsPerSample;
        let mix_sub = mix_ext.map(|e| {
            let sf = e.SubFormat;
            let mask = e.dwChannelMask;
            let ext_ch = e.Format.nChannels;
            (sf, mask, ext_ch)
        });

        let mix_is_f32 = match mix_sub {
            Some((sf, _, _)) => guid_eq(&sf, &SUBTYPE_IEEE_FLOAT),
            None => mix_tag == TAG_IEEE_FLOAT,
        };
        let mix_is_i16 = match mix_sub {
            Some((sf, _, _)) => guid_eq(&sf, &SUBTYPE_PCM) && mix_bits == 16,
            None => mix_tag == TAG_PCM && mix_bits == 16,
        };
        if !mix_is_f32 && !mix_is_i16 {
            return Err(HalError::Format(format!(
                "the mix format is neither f32 nor 16-bit PCM (tag {mix_tag:#06x}, {mix_bits}                  bits) — sparq converts exactly those two; please report this device"
            )));
        }

        let (hw_period, hw_period_min) = client.device_period();

        // The device's own channel mask, needed by every extensible probe below.
        let mask = match mix_sub {
            Some((_, m, ext_ch)) if ext_ch != 1 => m,
            _ => MASK_STEREO,
        };

        // Shared-mode rate envelope: probe f32 at each standard rate (the mixer accepts what it
        // accepts — probing is the only honest way to know). When EVERY probe is refused the
        // envelope below is fallback truth, not measured truth — say so, with the first HRESULT,
        // instead of letting a one-rate envelope masquerade as a measurement (defect #38: the
        // first SATURN log showed exactly that and hid the cause).
        let mut supported: Vec<u32> = Vec::new();
        let mut first_refusal: Option<HRESULT> = None;
        for &r in PROBE_RATES.iter() {
            let hr = client.probe_format(AUDCLNT_SHAREMODE_SHARED, &wfx_f32(r, mix_ch));
            // A plain-format refusal is not the whole answer: some endpoints accept only the
            // extensible form (defect #46), so probe that too before calling a rate unsupported.
            let ok =
                hr == 0 || client.format_supported_ext(AUDCLNT_SHAREMODE_SHARED, r, mix_ch, mask);
            if ok {
                supported.push(r);
            } else if first_refusal.is_none() {
                first_refusal = Some(hr);
            }
        }
        if supported.is_empty() {
            if let Some(hr) = first_refusal {
                eprintln!(
                    "sparq hal[{}]: IsFormatSupported refused EVERY rate probe (plain AND \
                     extensible f32) on this endpoint (first HRESULT {}) — capabilities below \
                     are the engine's mix-format fallback, not measurements; `open` will \
                     arbitrate via Initialize (docs/hal/windows-notes.md quirk log)",
                    self.kind,
                    hr_text(hr)
                );
            }
        }
        if !supported.contains(&mix_rate) {
            supported.push(mix_rate); // the mix rate always works in shared mode
        }
        supported.sort_unstable();
        supported.dedup();

        // Channel envelope: probe at the mix rate, both format forms.
        let mut ch_ok: Vec<u16> = Vec::new();
        for &c in PROBE_CHANNELS.iter() {
            if client.format_supported(AUDCLNT_SHAREMODE_SHARED, &wfx_f32(mix_rate, c))
                || client.format_supported_ext(AUDCLNT_SHAREMODE_SHARED, mix_rate, c, mask)
            {
                ch_ok.push(c);
            }
        }
        ch_ok.sort_unstable();
        ch_ok.dedup();
        let channels_out =
            (ch_ok.first().copied().unwrap_or(1), ch_ok.last().copied().unwrap_or(mix_ch).max(1));

        // Exclusive probes: the open ladder's four rungs (f32 → i24-in-32 → i32 → i16, all
        // extensible) at EVERY standard rate, with the device's own channel count/mask. A rate
        // counts when ANY rung is accepted — which rung actually opens is decided at open()
        // time and named in the log line. (Probing other channel counts is increment-2
        // territory; the acceptance run is stereo on the UMC204HD.)
        //
        // The sweep is deliberately NOT the shared-supported list (defect #91, test004 attempt
        // 3): the two envelopes answer different questions — shared asks the ENGINE (which on
        // SATURN's UMC 204HD refused every f32 rate probe except its own 96 kHz mix), exclusive
        // asks the DRIVER (which speaks i24-in-32 at the rates the hardware clocks at, 44.1 kHz
        // up). Sieving the exclusive envelope through the shared one printed `exclusive 96000`
        // as the measured truth while 48 kHz was never even asked — and play's rate adjustment
        // then bent a 48 kHz request UP into the one rate the exclusive path proved
        // pathological at. Each envelope is now probed with its own arbiter, the mix rate rides
        // along when the standard sweep does not name it, and a refusal here stays honest: the
        // open ladder still probes with Initialize as the final arbiter.
        let mut exclusive_rates: Vec<u32> = Vec::new();
        if self.exclusive_mode() {
            let mut sweep: Vec<u32> = PROBE_RATES.to_vec();
            if !sweep.contains(&mix_rate) {
                sweep.push(mix_rate);
            }
            for r in sweep {
                let f32e = wfxe_f32(r, mix_ch, mask);
                let i24 = wfxe_int(r, mix_ch, mask, 32, 24);
                let i32f = wfxe_int(r, mix_ch, mask, 32, 32);
                let i16f = wfxe_int(r, mix_ch, mask, 16, 16);
                let accepted = client.format_supported(AUDCLNT_SHAREMODE_EXCLUSIVE, &f32e.Format)
                    || client.format_supported(AUDCLNT_SHAREMODE_EXCLUSIVE, &i24.Format)
                    || client.format_supported(AUDCLNT_SHAREMODE_EXCLUSIVE, &i32f.Format)
                    || client.format_supported(AUDCLNT_SHAREMODE_EXCLUSIVE, &i16f.Format);
                if accepted {
                    exclusive_rates.push(r);
                }
            }
            exclusive_rates.sort_unstable();
            exclusive_rates.dedup();
        }
        let has_exclusive = !exclusive_rates.is_empty();

        Ok(Capabilities {
            rates: merge_rates(&supported),
            exclusive_rates,
            channels_out,
            channels_in: (0, 0), // increment 1: render only (module docs)
            default_rate: mix_rate,
            default_channels_out: mix_ch,
            exclusive: has_exclusive,
            event_driven: true, // WASAPI always supports EVENTCALLBACK; we always use it
            full_duplex: false, // increment 1 (module docs); the trait and null cover duplex
            hw_period,
            hw_period_min,
            f32_native: mix_is_f32,
        })
    }

    fn open(
        &self,
        dev: &DeviceId,
        cfg: &StreamConfig,
        opts: &OpenOptions,
        process: ProcessFn,
    ) -> Result<Box<dyn AudioStream>, HalError> {
        ensure_control_com()?;
        if cfg.inputs > 0 {
            return Err(HalError::Unsupported(format!(
                "{} input channel(s) requested, but full-duplex WASAPI is increment 2 of WO-006 \
                 (capture-endpoint pairing); use `--backend null` to exercise the duplex code \
                 path, or outputs-only for now",
                cfg.inputs
            )));
        }
        if cfg.outputs < 1 || cfg.outputs > 64 || cfg.block_frames < 1 {
            return Err(HalError::Format(format!(
                "config {} Hz / {} fr / {} out is outside the envelope (plan §9.6)",
                cfg.sample_rate, cfg.block_frames, cfg.outputs
            )));
        }
        let device = self.resolve(dev)?;
        let device_name = self
            .enumerate()?
            .into_iter()
            .find(|d| d.id == *dev)
            .map(|d| d.name)
            .unwrap_or_else(|| String::from("<resolved endpoint>"));

        let client = device.activate_client()?;
        let log = opts.log_negotiation;
        let (bundle, neg, event) = if self.exclusive_mode() {
            open_exclusive(&device, client, cfg, log)?
        } else {
            open_shared(client, cfg, log)?
        };

        // ---- buffers exist at open; start may not allocate (plan §5.2) ----------------
        let period = neg.period_frames as usize;
        let block = neg.block_frames as usize;
        let ch = neg.channels as usize;
        let fifo = Fifo::new((2 * period + 2 * block) * ch + 64);
        let mut block_scratch = vec![0.0f32; block * ch];
        for s in block_scratch.iter_mut() {
            *s = std::hint::black_box(0.0); // pre-touch: page faults belong to open
        }

        // One honest line per open — the first-run diagnostic that separates "sparq opened the
        // wrong thing" from "the device did the wrong thing" (the Phase A log lesson). Probe
        // opens stay silent (OpenOptions::log_negotiation = false) so the two-phase open does
        // not print every line twice and the conformance cycles do not bury their report.
        if log {
            // The open line is the proof of what the device got (the NEGOTIATED promise). When a
            // driver allocated something seriously smaller than the ask that opened — #89's
            // silent-shrink face — the ask rides along so the line cannot be misread as a clean
            // negotiation.
            let ask_note = match neg.ask_frames {
                Some(a) if a != neg.period_frames => format!(", ask {a} fr"),
                _ => String::new(),
            };
            eprintln!(
                "sparq hal[{}]: opened `{device_name}` — {} Hz · {} ch · {} · device period {} fr ({:.2} ms{ask_note}) · sparq block {} fr",
                self.kind,
                neg.rate,
                neg.channels,
                neg.fmt_name,
                neg.period_frames,
                f64::from(neg.period_frames) * 1000.0 / f64::from(neg.rate.max(1)),
                neg.block_frames,
            );
        }

        Ok(Box::new(WasapiStream {
            req: *cfg,
            neg,
            opts: opts.clone(),
            shared: Arc::new(PumpShared {
                stop: AtomicBool::new(false),
                state: AtomicU8::new(state_to_u8(StreamState::Idle)),
                last_hr: AtomicI64::new(0),
                event: AtomicIsize::new(event as isize),
            }),
            diag: Arc::new(DiagRecorder::new(neg.rate)),
            owned: Some(PumpOwned { bundle, proc: process, fifo, block: block_scratch }),
            pump: None,
        }))
    }
}

/// Merge sorted discrete rates into ranges. Audio rates are not contiguous, so in practice every
/// rate becomes its own singleton range — the honest representation: WASAPI accepts *discrete*
/// rates in exclusive mode, and a fake interval would promise 45 kHz works when it does not.
fn merge_rates(sorted: &[u32]) -> Vec<RateRange> {
    let mut out: Vec<RateRange> = Vec::new();
    for &r in sorted {
        match out.last_mut() {
            Some(last) if last.max + 1 == r => last.max = r,
            _ => out.push(RateRange { min: r, max: r }),
        }
    }
    out
}

/// Build a plain f32 `WAVEFORMATEX` (shared-mode probes and opens).
fn wfx_f32(rate: u32, ch: u16) -> WAVEFORMATEX {
    WAVEFORMATEX {
        wFormatTag: TAG_IEEE_FLOAT,
        nChannels: ch,
        nSamplesPerSec: rate,
        nAvgBytesPerSec: rate * u32::from(ch) * 4,
        nBlockAlign: ch * 4,
        wBitsPerSample: 32,
        cbSize: 0,
    }
}

/// Build an f32 `WAVEFORMATEXTENSIBLE` (exclusive mode requires the extensible form).
fn wfxe_f32(rate: u32, ch: u16, mask: u32) -> WAVEFORMATEXTENSIBLE {
    WAVEFORMATEXTENSIBLE {
        Format: WAVEFORMATEX {
            wFormatTag: TAG_EXTENSIBLE,
            nChannels: ch,
            nSamplesPerSec: rate,
            nAvgBytesPerSec: rate * u32::from(ch) * 4,
            nBlockAlign: ch * 4,
            wBitsPerSample: 32,
            cbSize: 22,
        },
        // Union initialisation (no unsafe needed): picking the wValidBitsPerSample member is the
        // documented interpretation for extensible f32 (full 32-bit validity).
        Samples: WAVEFORMATEXTENSIBLE_0 { wValidBitsPerSample: 32 },
        dwChannelMask: if ch == 1 {
            MASK_MONO
        } else if ch == 2 {
            MASK_STEREO
        } else {
            mask
        },
        SubFormat: SUBTYPE_IEEE_FLOAT,
    }
}

/// Build an integer `WAVEFORMATEXTENSIBLE` (exclusive mode requires the extensible form).
/// `container` is the storage width (32 or 16); `valid` the meaningful bits — 24-in-32 is
/// container 32 / valid 24, left-justified per the ksmedia convention, which is why the pump's
/// i32 conversion serves both 32-bit rungs unchanged.
fn wfxe_int(rate: u32, ch: u16, mask: u32, container: u16, valid: u16) -> WAVEFORMATEXTENSIBLE {
    debug_assert!(valid <= container && (container == 32 || container == 16));
    let align = ch * container / 8;
    WAVEFORMATEXTENSIBLE {
        Format: WAVEFORMATEX {
            wFormatTag: TAG_EXTENSIBLE,
            nChannels: ch,
            nSamplesPerSec: rate,
            nAvgBytesPerSec: rate * u32::from(align),
            nBlockAlign: align,
            wBitsPerSample: container,
            cbSize: 22,
        },
        // Union initialisation (no unsafe needed): the valid-bits member is the documented
        // interpretation for PCM extensible formats.
        Samples: WAVEFORMATEXTENSIBLE_0 { wValidBitsPerSample: valid },
        dwChannelMask: if ch == 1 {
            MASK_MONO
        } else if ch == 2 {
            MASK_STEREO
        } else {
            mask
        },
        SubFormat: SUBTYPE_PCM,
    }
}

/// The exclusive open: the four-rung format ladder (f32 → i24-in-32 → i32 → i16, all
/// extensible) at the requested rate/channels, event-driven, with the documented
/// `BUFFER_SIZE_NOT_ALIGNED` two-step (rounding the ask UP to the driver's granularity — #89).
/// Returns bundle + negotiated truth + event.
fn open_exclusive(
    device: &Com<MmDevice>,
    first_client: Com<AudioClient>,
    cfg: &StreamConfig,
    log: bool,
) -> Result<(PumpBundle, Negotiated, HANDLE), HalError> {
    let rate = cfg.sample_rate.max(1);
    let ch = cfg.outputs as u16;

    // Channel mask: the device's own when the channel count matches, else mono/stereo by count.
    // (Packed-struct rule: copy fields to locals; never reference them.)
    let mask = first_client
        .mix_format()
        .ok()
        .and_then(|(mix, ext)| {
            let tag = mix.wFormatTag;
            ext.and_then(|e| {
                let ext_ch = e.Format.nChannels;
                let m = e.dwChannelMask;
                (ext_ch == ch).then_some(m)
            })
            .or_else(|| {
                (tag == TAG_EXTENSIBLE).then_some(if ch == 1 { MASK_MONO } else { MASK_STEREO })
            })
        })
        .unwrap_or(if ch == 1 { MASK_MONO } else { MASK_STEREO });

    // The exclusive ladder (defect #77): f32 first — zero conversion, the format sparq thinks
    // in — then the integer rungs for the USB DAC drivers that refuse float in exclusive. The
    // first S_OK wins; S_FALSE ("here is the closest match") is a NO, exactly as
    // `format_supported` treats it. All refusals are collected so the error carries the full
    // probe table (the #38/#46 lesson: a refusal that cannot be diagnosed from the log costs a
    // device round-trip).
    /// One rung of the ladder: the pump format it implies, its name in logs/errors, and the
    /// integer widths `(container_bits, valid_bits)` — `None` for the float rung.
    type ExclRung = (DevFmt, &'static str, Option<(u16, u16)>);
    let rung_specs: [ExclRung; 4] = [
        (DevFmt::F32, "f32", None),
        (DevFmt::I32, "i24-in-32 (converting)", Some((32, 24))),
        (DevFmt::I32, "i32 (converting)", Some((32, 32))),
        (DevFmt::I16, "i16 (converting)", Some((16, 16))),
    ];
    let mut chosen: Option<(WAVEFORMATEXTENSIBLE, DevFmt, &'static str)> = None;
    let mut probe_log: Vec<String> = Vec::new();
    for (fmt, name, int_bits) in rung_specs {
        let candidate = match int_bits {
            None => wfxe_f32(rate, ch, mask),
            Some((container, valid)) => wfxe_int(rate, ch, mask, container, valid),
        };
        let hr = first_client.probe_format(AUDCLNT_SHAREMODE_EXCLUSIVE, &candidate.Format);
        if hr == 0 {
            chosen = Some((candidate, fmt, name));
            break;
        }
        probe_log.push(format!("{name}: {}", hr_text(hr)));
    }
    let Some((wfxe, dev_fmt, fmt_name)) = chosen else {
        return Err(HalError::Format(format!(
            "exclusive at {rate} Hz / {ch} ch is not supported by this endpoint in any format \
             sparq speaks — probe table [{}] (run `sparq devices --caps` for the verified \
             exclusive rates; shared mode may still work: --backend wasapi-shared; \
             remote-desktop endpoints never allow exclusive — that is the OS, not sparq)",
            probe_log.join(" · ")
        )));
    };

    // Period negotiation (defects #79 + #89): the block period is a REQUEST, not an entitlement —
    // and neither is the driver's own reported minimum. Inc 1.2 asked Initialize for the sparq
    // block — 64 frames = 666 µs at 96 kHz — and the Behringer's 10 ms engine answered
    // AUDCLNT_E_INVALID_DEVICE_PERIOD (#79): an honest refusal the format ladder cannot see
    // (IsFormatSupported takes no period). Inc 1.3 then asked the min-clamped block period first,
    // and test004 attempt 2 (2026-09-28) proved the worse failure mode: the UMC 204HD ACCEPTED
    // its reported 3 ms minimum and could not SUSTAIN it — events every ~5.9 ms instead of every
    // 3 ms, 159 stalls of ~33 ms in 10 s, half throughput, drift −49 %, no clean tone — while the
    // same machine's shared 10 ms engine soak-ran 2 h clean (#89). An accepted Initialize is not a
    // promise the period will run; the DEFAULT period is the number the driver's engine actually
    // runs at. The ladder is pure data from `crate::hal::period` (unit-tested on Linux, where this
    // project is developed): for a coarser-than-block engine the default goes first and the
    // min-clamped ask is the fallback; for a sub-block engine the honest low-latency ask keeps
    // priority, because a lying driver there refuses at Initialize *honestly* and the default
    // catches it. Each candidate gets a FRESH IAudioClient — a failed Initialize consumes it —
    // and the documented BUFFER_SIZE_NOT_ALIGNED two-step rides along per candidate, with #89's
    // rounding: the granularity it reports rounds the ask UP, never replaces it. The pump's FIFO
    // already decouples device period from sparq block ("Period ≠ block",
    // docs/hal/windows-notes.md), so accepting the driver's period changes nothing structurally;
    // the latency report and the open log line read the TRUE period from GetBufferSize in
    // finish_open, as before — and the open line now also carries the ask whenever the driver
    // allocated something seriously smaller than what it accepted.
    let (hw_default, hw_min) = first_client.device_period();
    let to_100ns = |d: Option<Duration>| d.map(|d| (d.as_nanos() / 100) as i64);
    let ladder = crate::hal::period::exclusive_period_ladder(
        cfg.block_frames as u32,
        rate,
        to_100ns(hw_default),
        to_100ns(hw_min),
    );
    let mut period_log: Vec<String> = Vec::new();
    let mut client = first_client;
    let mut first_try = true;
    let mut opened: Option<(Com<AudioClient>, i64)> = None;
    for &cand in &ladder {
        if !first_try {
            client = device.activate_client()?;
        }
        first_try = false;
        // The ask that would open right now: starts as the candidate, follows the alignment
        // two-step and the shrink retry, so the Negotiated truth and the log always name the
        // number the successful Initialize actually got.
        let mut ask = cand;
        let mut hr = client.initialize(
            AUDCLNT_SHAREMODE_EXCLUSIVE,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
            ask,
            ask,
            &wfxe.Format,
        );
        let mut tried = format!("{}: {}", crate::hal::period::period_text(cand), hr_text(hr));
        if hr == AUDCLNT_E_BUFFER_SIZE_NOT_ALIGNED {
            // Alignment two-step (documented recovery) with #89's rounding: the failed client
            // reports the driver's alignment granularity via GetBufferSize, and the re-ask is the
            // ORIGINAL period rounded UP to that granularity — never the granularity itself. Inc
            // 1.3 followed the recipe literally and turned the UMC's 10 ms ask into a 3 ms open:
            // aligned, accepted — and unsustainable at runtime. An ask below one unit still lands
            // on one unit, the behaviour the recipe exists for. A FRESH client is activated
            // because an initialised (or failed-initialised) IAudioClient cannot be re-initialised.
            let gran = client.buffer_size()?;
            let up_frames = crate::hal::period::align_up_frames(
                crate::hal::period::period_100ns_to_frames(cand, rate),
                gran,
            );
            let aligned = crate::hal::period::frames_to_100ns(up_frames, rate);
            client = device.activate_client()?;
            ask = aligned;
            hr = client.initialize(
                AUDCLNT_SHAREMODE_EXCLUSIVE,
                AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                aligned,
                aligned,
                &wfxe.Format,
            );
            tried = format!(
                "{tried} → aligned-up {}: {}",
                crate::hal::period::period_text(aligned),
                hr_text(hr)
            );
        }
        if hr >= 0 {
            // #89's third face: some stacks ACCEPT the ask and then report a seriously smaller
            // allocation through GetBufferSize — the same granularity story, told silently. The
            // allocation is then the driver's granularity: round the ask up to it and re-ask
            // ONCE on a fresh client. If the retry opens, it wins; if it refuses (or activation
            // fails), the original open is kept — a stream that opened with the mismatch printed
            // on the open line beats a refusal we can diagnose but not hear.
            let asked_frames = crate::hal::period::period_100ns_to_frames(ask, rate);
            let allocated = client.buffer_size().unwrap_or(0);
            if crate::hal::period::allocation_seriously_shrunk(asked_frames, allocated) {
                let up_frames = crate::hal::period::align_up_frames(asked_frames, allocated.max(1));
                let up = crate::hal::period::frames_to_100ns(up_frames, rate);
                // If activation fails, keep the open that exists; the mismatch rides the open
                // line rather than turning a working (if shrunken) stream into a refusal.
                if let Ok(retry) = device.activate_client() {
                    let hr2 = retry.initialize(
                        AUDCLNT_SHAREMODE_EXCLUSIVE,
                        AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                        up,
                        up,
                        &wfxe.Format,
                    );
                    if hr2 >= 0 {
                        if log {
                            eprintln!(
                                "sparq hal[wasapi-exclusive]: driver allocated {allocated} fr against a {} ask — treated the allocation as its granularity and re-asked {}: opened. Please send this line back.",
                                crate::hal::period::period_text(ask),
                                crate::hal::period::period_text(up),
                            );
                        }
                        client = retry;
                        ask = up;
                    } else if log {
                        eprintln!(
                            "sparq hal[wasapi-exclusive]: driver allocated {allocated} fr against a {} ask; the rounded-up re-ask {} was refused ({}) — keeping the shrunken open, and the open line will say so. Please send this line back.",
                            crate::hal::period::period_text(ask),
                            crate::hal::period::period_text(up),
                            hr_text(hr2),
                        );
                    }
                }
            }
            opened = Some((client, ask));
            break;
        }
        period_log.push(tried);
        if hr != AUDCLNT_E_INVALID_DEVICE_PERIOD {
            // Busy / invalidated / not-allowed: no other period will fix that. Stop asking and
            // report — the table says what the endpoint actually answered.
            break;
        }
    }
    let Some((client, ask)) = opened else {
        return Err(HalError::Device(format!(
            "Initialize (exclusive) was refused at {rate} Hz / {ch} ch ({fmt_name}) for every \
             device period tried [{}] — endpoint reports default {}, minimum {}; if this is a \
             USB interface its driver wants a period this ladder does not offer — send this \
             line back, the table IS the diagnosis",
            period_log.join(" · "),
            to_100ns(hw_default)
                .map(crate::hal::period::period_text)
                .unwrap_or_else(|| "unknown".to_string()),
            to_100ns(hw_min)
                .map(crate::hal::period::period_text)
                .unwrap_or_else(|| "unknown".to_string()),
        )));
    };

    finish_open(
        client,
        Negotiated {
            exclusive: true,
            rate,
            channels: ch,
            period_frames: 0, // filled by finish_open from GetBufferSize
            ask_frames: Some(crate::hal::period::period_100ns_to_frames(ask, rate)),
            block_frames: cfg.block_frames as u32,
            fmt: dev_fmt,
            fmt_name,
        },
    )
}

/// The shared open: probe ladder f32@requested → f32@mix-rate → mix-as-is (i16 → the pump
/// converts). The engine owns the period; `GetBufferSize` after `Initialize` tells the truth.
/// The format a shared-mode open settled on: either a rebuilt plain WAVEFORMATEX or the
/// engine's own mix format kept verbatim (which may be the extensible form — passing a bare
/// WAVEFORMATEX copy of an extensible format to Initialize would make the driver read past the
/// struct, so the two shapes stay distinct all the way down).
enum Chosen {
    Plain(WAVEFORMATEX),
    Ext(WAVEFORMATEXTENSIBLE),
}

impl Chosen {
    fn wfx(&self) -> &WAVEFORMATEX {
        match self {
            Self::Plain(w) => w,
            Self::Ext(e) => &e.Format,
        }
    }
}

fn open_shared(
    client: Com<AudioClient>,
    cfg: &StreamConfig,
    log: bool,
) -> Result<(PumpBundle, Negotiated, HANDLE), HalError> {
    // WAVEFORMATEX(TENSIBLE) are packed(1) in windows-sys: copy every used field to a plain
    // local immediately (referencing a packed field is UB). The structs themselves stay alive
    // as locals: rung 6 offers the mix format *verbatim*, extension bytes included.
    let (mix, mix_ext) = client.mix_format()?;
    let mix_rate = mix.nSamplesPerSec;
    let mix_ch = mix.nChannels;
    let mix_tag = mix.wFormatTag;
    let mix_bits = mix.wBitsPerSample;
    let mix_sub = mix_ext.map(|e| e.SubFormat);
    let mix_f32 = match mix_sub {
        Some(sf) => guid_eq(&sf, &SUBTYPE_IEEE_FLOAT),
        None => mix_tag == TAG_IEEE_FLOAT,
    };
    let mix_i16 = match mix_sub {
        Some(sf) => guid_eq(&sf, &SUBTYPE_PCM) && mix_bits == 16,
        None => mix_tag == TAG_PCM && mix_bits == 16,
    };
    if !mix_f32 && !mix_i16 {
        return Err(HalError::Format(format!(
            "the mix format is neither f32 nor 16-bit PCM (tag {mix_tag:#06x}, {mix_bits} bits)              — sparq converts exactly those two; please report this device"
        )));
    }

    let ch = (cfg.outputs as u16).max(1);
    let mask = match mix_ext {
        Some(e) => {
            let m = e.dwChannelMask;
            let ec = e.Format.nChannels;
            if ec == ch {
                m
            } else if ch == 1 {
                MASK_MONO
            } else {
                MASK_STEREO
            }
        },
        None => {
            if ch == 1 {
                MASK_MONO
            } else {
                MASK_STEREO
            }
        },
    };
    // The ladder, in order of preference. Every refusal is recorded *with its HRESULT* —
    // "the probe said no" without the code is how the first SATURN run became a guessing game
    // (defect #38): some stacks (observed: the RDP `Remote Audio` endpoint) refuse EVERY
    // IsFormatSupported, apparently including their own mix format, while Initialize with that
    // very format works fine. The probe is advisory; Initialize is the truth.
    //
    //   rungs 1-2: f32 at the requested rate/channels — plain, then EXTENSIBLE shape
    //   rungs 3-4: f32 at the mix rate (shared engines reject off-rate opens; the mixer
    //              resamples) — plain, then extensible
    //   rung 5: rebuilt plain mix-shape format (i16 mix → the pump converts); skipped when the
    //           mix is extensible — rung 6 offers the real thing, extension bytes included
    //   rung 6: the engine's own mix format, verbatim
    //   rung 7 (not probed): the Initialize-with-mix last resort inside the None arm below
    //
    // Both shapes at every level because some endpoints refuse the plain WAVEFORMATEX and accept
    // only WAVEFORMATEXTENSIBLE (defect #46 — the SATURN/RDP endpoint refused every plain probe
    // with AUDCLNT_E_UNSUPPORTED_FORMAT, then accepted its extensible mix verbatim). Probing one
    // shape reads a driver preference as a device limitation.
    let req_rate = cfg.sample_rate.max(1);
    let mut candidates: Vec<(u8, String, Chosen, u32, u16, bool)> = vec![
        (
            1,
            format!("f32 {req_rate} Hz/{ch} ch (requested)"),
            Chosen::Plain(wfx_f32(req_rate, ch)),
            req_rate,
            ch,
            true,
        ),
        (
            2,
            format!("f32-ext {req_rate} Hz/{ch} ch (requested, extensible)"),
            Chosen::Ext(wfxe_f32(req_rate, ch, mask)),
            req_rate,
            ch,
            true,
        ),
        (
            3,
            format!("f32 {mix_rate} Hz/{ch} ch (mix rate)"),
            Chosen::Plain(wfx_f32(mix_rate, ch)),
            mix_rate,
            ch,
            true,
        ),
        (
            4,
            format!("f32-ext {mix_rate} Hz/{ch} ch (mix rate, extensible)"),
            Chosen::Ext(wfxe_f32(mix_rate, ch, mask)),
            mix_rate,
            ch,
            true,
        ),
    ];
    if mix_ext.is_none() {
        let (desc, fmt) = if mix_f32 {
            (
                format!("f32 {mix_rate} Hz/{mix_ch} ch (mix shape, rebuilt)"),
                Chosen::Plain(wfx_f32(mix_rate, mix_ch)),
            )
        } else {
            // PCM16 layout is fully determined by rate/channels: nBlockAlign 2×ch,
            // nAvgBytesPerSec rate×that.
            (
                format!("i16 {mix_rate} Hz/{mix_ch} ch (mix shape, rebuilt)"),
                Chosen::Plain(WAVEFORMATEX {
                    wFormatTag: TAG_PCM,
                    nChannels: mix_ch,
                    nSamplesPerSec: mix_rate,
                    nAvgBytesPerSec: mix_rate * u32::from(mix_ch) * 2,
                    nBlockAlign: mix_ch * 2,
                    wBitsPerSample: 16,
                    cbSize: 0,
                }),
            )
        };
        candidates.push((5, desc, fmt, mix_rate, mix_ch, mix_f32));
    }
    candidates.push((
        6,
        format!("{mix_rate} Hz/{mix_ch} ch (mix format VERBATIM, f32={mix_f32})"),
        match &mix_ext {
            Some(e) => Chosen::Ext(*e),
            None => Chosen::Plain(mix),
        },
        mix_rate,
        mix_ch,
        mix_f32,
    ));

    let mut rungs: Vec<(String, HRESULT)> = Vec::new();
    let mut chosen: Option<(Chosen, u32, u16, bool, u8)> = None;
    for (rung, desc, fmt, rate, channels, f32) in candidates {
        let hr = client.probe_format(AUDCLNT_SHAREMODE_SHARED, fmt.wfx());
        if hr == 0 {
            chosen = Some((fmt, rate, channels, f32, rung));
            break;
        }
        rungs.push((format!("rung {rung}: {desc}"), hr));
    }

    // ---- negotiate: the chosen rung, or Initialize-with-mix as the last-resort arbiter -----
    let rung_table = || {
        rungs
            .iter()
            .map(|(d, hr)| format!("    {d} -> {}", hr_text(*hr)))
            .collect::<Vec<_>>()
            .join("\n")
    };
    let (chosen_fmt, rate, channels, f32, rung) = match chosen {
        Some(c) => c,
        None => {
            // Every probe refused — including (on the observed RDP endpoint) the engine's own
            // mix format. Initialize is the authority that matters; try it with the mix verbatim
            // before declaring defeat. This is not a silent fallback: it is logged, and the
            // probe HRs ride along in the success log and the failure message.
            if log {
                eprintln!(
                    "sparq hal[wasapi-shared]: IsFormatSupported refused EVERY probe —\n{}\n  \
                     falling back to Initialize with the engine's own mix format (the probe is \
                     advisory; Initialize is the truth). Driver quirk — please send this log back.",
                    rung_table()
                );
            }
            let fallback = match &mix_ext {
                Some(e) => Chosen::Ext(*e),
                None => Chosen::Plain(mix),
            };
            let period_100ns = (cfg.block_frames as i64) * 10_000_000 / i64::from(mix_rate.max(1));
            let hr = client.initialize(
                AUDCLNT_SHAREMODE_SHARED,
                AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
                period_100ns.max(30_000),
                0,
                fallback.wfx(),
            );
            if hr < 0 {
                return Err(HalError::Format(format!(
                    "shared mode refused every ladder rung AND the Initialize fallback \
                     (requested {} Hz/{ch} ch; mix {mix_rate} Hz/{mix_ch} ch, f32={mix_f32})\n\
                     probes:\n{}\n  fallback Initialize: {}\n  run `sparq devices --caps` \
                     and report this device",
                    cfg.sample_rate,
                    rung_table(),
                    hr_text(hr)
                )));
            }
            if log {
                eprintln!(
                    "sparq hal[wasapi-shared]: fallback Initialize ACCEPTED the mix format \
                     ({mix_rate} Hz/{mix_ch} ch, f32={mix_f32}) — proceeding"
                );
            }
            (fallback, mix_rate, mix_ch, mix_f32, 7)
        },
    };
    if rung != 1 && log {
        eprintln!(
            "sparq hal[wasapi-shared]: requested {} Hz/{ch} ch refused; negotiated rung {rung} \
             ({rate} Hz/{channels} ch, f32={f32}) — the mixer resamples; the latency report \
             reflects the negotiated truth",
            cfg.sample_rate
        );
    }

    if rung != 7 {
        // Rung 7 already initialised (inside the fallback). Everyone else initialises now.
        let period_100ns = (cfg.block_frames as i64) * 10_000_000 / i64::from(rate.max(1));
        // Shared buffers below ~3 ms are rejected (or silently rounded up) by most engines;
        // asking for at least 3 ms keeps GetBufferSize's answer honest.
        let hr = client.initialize(
            AUDCLNT_SHAREMODE_SHARED,
            AUDCLNT_STREAMFLAGS_EVENTCALLBACK,
            period_100ns.max(30_000),
            0,
            chosen_fmt.wfx(),
        );
        if hr < 0 {
            return Err(classify("Initialize (shared)", hr));
        }
    }

    finish_open(
        client,
        Negotiated {
            exclusive: false,
            rate,
            channels,
            period_frames: 0,
            // Shared mode: the engine picks its own buffer; an ask annotation would be noise
            // (the requested period there is a hint the engine is documented to ignore).
            ask_frames: None,
            block_frames: cfg.block_frames as u32,
            fmt: if f32 { DevFmt::F32 } else { DevFmt::I16 },
            fmt_name: if f32 { "f32" } else { "i16 (mix, converting)" },
        },
    )
}

/// Shared tail of both opens: event handle, services, the true device period, the bundle.
/// On any failure the event handle is closed before returning — no leaked kernel objects.
fn finish_open(
    client: Com<AudioClient>,
    mut neg: Negotiated,
) -> Result<(PumpBundle, Negotiated, HANDLE), HalError> {
    // Auto-reset event: the documented WASAPI event-driven pattern (one signal per period).
    // SAFETY: null attributes/name, auto-reset (FALSE), initially non-signalled (FALSE) — a
    // plain kernel event owned by the stream from here until its Drop closes it.
    let event = unsafe { CreateEventW(ptr::null(), 0, 0, ptr::null()) };
    if event.is_null() {
        return Err(HalError::System(String::from("CreateEventW failed (out of handles?)")));
    }

    if let Err(e) = client.set_event_handle(event) {
        // SAFETY: owned handle, open failed, no stream will exist — close exactly once here.
        unsafe {
            let _ = CloseHandle(event);
        }
        return Err(e);
    }
    let render = match client.render_client() {
        Ok(r) => r,
        Err(e) => {
            // SAFETY: owned handle, open failed — close exactly once here.
            unsafe {
                let _ = CloseHandle(event);
            }
            return Err(e);
        },
    };
    // The drift metric no longer reads a device clock (defect #76: GetPosition on these
    // endpoints advances in buffer-sized steps per event tick - see the note where the
    // IAudioClock binding used to be). The pump counts the frames the device ACCEPTS instead:
    // every ReleaseBuffer is an exact, local truth, on every driver.
    let period_frames = match client.buffer_size() {
        Ok(f) => f.max(1),
        Err(e) => {
            // SAFETY: owned handle, open failed — close exactly once here (render/client drop
            // via Com).
            unsafe {
                let _ = CloseHandle(event);
            }
            return Err(e);
        },
    };
    neg.period_frames = period_frames;
    Ok((PumpBundle { client, render }, neg, event))
}

// --------------------------------------------------------------------------- the stream

/// A WASAPI stream. Everything device-facing exists from `open`; `start` only moves ownership to
/// the pump thread and waits (bounded) for the driver to accept the start.
pub struct WasapiStream {
    req: StreamConfig,
    neg: Negotiated,
    opts: OpenOptions,
    shared: Arc<PumpShared>,
    diag: Arc<DiagRecorder>,
    /// `None` only while the pump thread owns it (moved back on join — nothing leaks).
    owned: Option<PumpOwned>,
    pump: Option<JoinHandle<PumpOwned>>,
}

impl WasapiStream {
    fn state(&self) -> StreamState {
        state_from_u8(self.shared.state.load(Ordering::Acquire))
    }

    fn last_error_text(&self) -> Option<String> {
        match self.state() {
            StreamState::Removed => Some(String::from(
                "the endpoint was invalidated (unplugged, or its driver reset) — re-enumerate \
                 (`sparq devices`) and open again",
            )),
            StreamState::Failed => {
                let hr = self.shared.last_hr.load(Ordering::Relaxed);
                Some(if hr != 0 {
                    classify("pump failure", hr as HRESULT).to_string()
                } else {
                    String::from("the pump thread failed before initialising (see stderr)")
                })
            },
            _ => None,
        }
    }
}

impl AudioStream for WasapiStream {
    fn start(&mut self) -> Result<(), HalError> {
        let now = self.state();
        match now {
            StreamState::Running | StreamState::Starting => {
                return Err(HalError::Device(String::from(
                    "start on a running stream: lifecycle bug — stop it first",
                )));
            },
            StreamState::Removed | StreamState::Failed => {
                return Err(HalError::Removed(format!(
                    "stream is in state {now:?}: {}",
                    self.last_error_text().unwrap_or_default()
                )));
            },
            StreamState::Idle | StreamState::Stopped => {},
        }
        let Some(mut owned) = self.owned.take() else {
            return Err(HalError::System(String::from(
                "pump already owns the stream resources (or a previous pump panicked)",
            )));
        };
        owned.fifo.reset();
        if self.shared.event.load(Ordering::Relaxed) == 0 {
            self.owned = Some(owned);
            return Err(HalError::System(String::from(
                "internal: the stream lost its event handle (open bug) — reopen the stream",
            )));
        }

        self.shared.stop.store(false, Ordering::Release);
        self.shared.state.store(state_to_u8(StreamState::Starting), Ordering::Release);

        let shared = Arc::clone(&self.shared);
        let diag = Arc::clone(&self.diag);
        let neg = self.neg;
        let opts = self.opts.clone();
        let builder = std::thread::Builder::new().name(String::from("sparq-wasapi-pump"));
        let handle =
            builder.spawn(move || wasapi_pump(owned, shared, diag, neg, opts)).map_err(|e| {
                self.shared.state.store(state_to_u8(StreamState::Failed), Ordering::Release);
                HalError::System(format!("spawning the WASAPI pump thread: {e}"))
            })?;
        self.pump = Some(handle);

        // Wait (bounded) for the pump to report Running/Failed/Removed: start() must not return
        // Ok on a stream that will never play — the CLI branches on this.
        let deadline = Instant::now() + Duration::from_secs(5);
        while Instant::now() < deadline {
            match self.state() {
                StreamState::Starting => std::thread::sleep(Duration::from_millis(5)),
                StreamState::Running => return Ok(()),
                StreamState::Removed => {
                    return Err(HalError::Removed(
                        self.last_error_text().unwrap_or_else(|| String::from("device removed")),
                    ));
                },
                _ => {
                    return Err(HalError::Device(
                        self.last_error_text().unwrap_or_else(|| String::from("start failed")),
                    ));
                },
            }
        }
        Err(HalError::System(String::from(
            "the pump did not reach Running within 5 s (driver hung during Start?) — call stop() \
             and reopen; if this repeats, run scripts\\diag.bat and send the log",
        )))
    }

    fn stop(&mut self) -> Result<(), HalError> {
        if let Some(handle) = self.pump.take() {
            self.shared.stop.store(true, Ordering::Release);
            // Wake the pump out of WaitForSingleObject without waiting for the next device period.
            let ev = self.shared.event.load(Ordering::Relaxed);
            if ev != 0 {
                // SAFETY: the handle is stream-owned and valid until Drop (which joins the pump
                // first); SetEvent on a valid auto-reset event is a pure kernel call. A racing
                // pump exit is benign: SetEvent either wakes it or is a no-op, and Drop cannot
                // run before this join completes.
                unsafe {
                    let _ = SetEvent(ev as HANDLE);
                }
            }
            match handle.join() {
                Ok(owned) => self.owned = Some(owned),
                Err(_) => {
                    self.shared.state.store(state_to_u8(StreamState::Failed), Ordering::Release);
                    self.diag.record_device_error();
                    // The pump panicked holding the bundle: the COM interfaces Release inside
                    // that thread's unwinding (Com<T>::drop), and `owned` stays None — start()
                    // will refuse honestly instead of pretending to hold resources.
                    return Err(HalError::System(String::from(
                        "the WASAPI pump thread panicked; the stream is Failed and disposable",
                    )));
                },
            }
        }
        let s = self.state();
        if s == StreamState::Running || s == StreamState::Starting {
            self.shared.state.store(state_to_u8(StreamState::Stopped), Ordering::Release);
        }
        Ok(())
    }

    fn is_running(&self) -> bool {
        self.state() == StreamState::Running
    }

    fn config(&self) -> StreamConfig {
        self.req
    }

    fn actual_config(&self) -> StreamConfig {
        StreamConfig {
            sample_rate: self.neg.rate,
            block_frames: self.neg.block_frames as usize,
            inputs: 0,
            outputs: self.neg.channels as usize,
            exclusive: self.neg.exclusive,
        }
    }

    fn latency_report(&self) -> LatencyReport {
        // Pipeline estimate: one sparq block (callback → FIFO) + one device period (FIFO → DAC).
        // The measured round trip (loopback impulse) is increment 2; until then `None`, never a
        // guess dressed as a measurement.
        LatencyReport {
            sample_rate: self.neg.rate,
            block_frames: self.neg.block_frames,
            hw_period_frames: Some(self.neg.period_frames),
            output_pipeline_frames: self.neg.block_frames + self.neg.period_frames,
            input_pipeline_frames: 0,
            measured_roundtrip_frames: None,
        }
    }

    fn error_report(&self) -> StreamErrorReport {
        StreamErrorReport {
            state: self.state(),
            last_error: self.last_error_text(),
            device_errors: self.diag.snapshot().device_errors,
            rt: self.diag.rt_report(),
        }
    }

    fn diag(&self) -> crate::hal::diag::DiagSnapshot {
        self.diag.snapshot()
    }

    fn pump(&mut self, _blocks: u64) -> Result<u64, HalError> {
        Err(HalError::Unsupported(String::from(
            "hardware paces itself; manual stepping exists on the null backend (`--backend null` \
             with OpenOptions::manual_with_capture)",
        )))
    }

    fn inject_fault(&mut self, fault: Fault) -> Result<(), HalError> {
        // Honest refusal, per the WO-006 evidence list: real-driver fault paths are tested by
        // really unplugging things (the unplug test), not by simulating them here.
        Err(HalError::Unsupported(format!(
            "fault {fault:?} cannot be injected into a real driver; the null backend simulates \
             it for CI, and the WO-006 unplug test covers this backend on hardware",
        )))
    }
}

impl Drop for WasapiStream {
    fn drop(&mut self) {
        if self.pump.is_some() {
            let _ = AudioStream::stop(self);
        }
        let ev = self.shared.event.swap(0, Ordering::AcqRel);
        if ev != 0 {
            // SAFETY: the pump was joined above, so no thread waits on this handle; the stream
            // owned it since open(); the swap-to-0 makes a double close impossible.
            unsafe {
                let _ = CloseHandle(ev as HANDLE);
            }
        }
    }
}

// --------------------------------------------------------------------------- the pump

/// The event-driven pump. Returns everything it was given so the stream can restart or drop
/// without leaking.
fn wasapi_pump(
    mut owned: PumpOwned,
    shared: Arc<PumpShared>,
    diag: Arc<DiagRecorder>,
    neg: Negotiated,
    opts: OpenOptions,
) -> PumpOwned {
    // COM + RT discipline on this thread, before anything touches the device.
    let com = match PumpCom::new() {
        Ok(c) => c,
        Err(e) => {
            eprintln!("sparq hal: pump thread COM init failed: {e}");
            shared.state.store(state_to_u8(StreamState::Failed), Ordering::Release);
            return owned;
        },
    };
    let period = Duration::from_secs_f64(f64::from(neg.period_frames) / f64::from(neg.rate.max(1)));
    let rt = RtThreadGuard::apply(
        opts.ideal_processor,
        if opts.working_set_lock { owned.fifo.buf.len() * 4 + owned.block.len() * 4 } else { 0 },
    );
    diag.set_rt_report(rt.report());

    let period_frames = neg.period_frames as usize;
    let ch = neg.channels as usize;
    let block = neg.block_frames as usize;
    let clock = Clock::new(neg.rate, 120.0, 960);
    let mut ctx = BlockContext {
        block: BlockId::FIRST,
        sample_offset: 0,
        frames: block,
        sample_rate: neg.rate,
        channels: ch,
        tick: 0,
        ppqn: 960,
    };
    let budget = Duration::from_secs_f64(block as f64 / f64::from(neg.rate.max(1)));
    // Frames the device has ACCEPTED: every frame handed over by a successful ReleaseBuffer,
    // preroll included. This count — not a clock read — is the drift metric's numerator (#76:
    // IAudioClock's GetPosition advances in buffer-sized steps per event tick on these
    // endpoints; the pump's own release count is exact on every driver, and a pump that
    // under-feeds a running device tells that truth through the xrun counters beside it).
    let mut delivered_frames = 0u64;
    let start = Instant::now();

    // One callback block: audit + budget + ctx advance + FIFO push. Shared by prime and loop.
    macro_rules! run_block {
        () => {{
            let guard = opts.audit_allocations.then(CountingGuard::new);
            let t0 = Instant::now();
            let mut fctx = FrameCtx { block: ctx, out: &mut owned.block, input: &[] };
            let status = (owned.proc)(&mut fctx);
            let dur = t0.elapsed();
            let allocs = guard.map_or(0, CountingGuard::finish);
            diag.record_callback(dur, status, Some(budget), allocs);
            owned.fifo.push(&owned.block);
            ctx = ctx.advance(&clock);
            status
        }};
    }

    // Prime the FIFO with TWO device periods + a block of cushion BEFORE Start (defect #43:
    // priming only P+B left exactly B samples after the preroll wrote P, so the first event —
    // which wants a full period — was guaranteed to starve and count a startup xrun. Every
    // SATURN run showed exactly that one xrun and nothing else; the soak fail-fast tripped on
    // it inside the first second).
    let prime_target = (2 * period_frames + block) * ch;
    while owned.fifo.len() < prime_target {
        run_block!();
    }

    // Preroll the device buffer, then Start. (Event-driven preroll: fill once; the first event
    // arrives one period later.) The FIFO was just primed above one full period, so the write
    // cannot starve; all raw-pointer discipline lives inside write_from_fifo.
    let preroll =
        owned.bundle.render.write_from_fifo(neg.period_frames, ch, neg.fmt, &mut owned.fifo).map(
            |_| {
                // The whole preroll was RELEASED to the device buffer — accepted whether or not
                // the FIFO could fill it (a shortfall went out as silence). Counting it keeps the
                // first in-loop drift reading from opening one period short of the truth.
                delivered_frames += u64::from(neg.period_frames);
            },
        );
    if let Err(e) = preroll {
        // Some shared-mode stacks refuse GetBuffer before Start; not fatal — the first event
        // writes a full period anyway (a starvation xrun is counted if it comes to that).
        eprintln!("sparq hal: preroll skipped: {e}");
    }

    if let Err(e) = owned.bundle.client.start() {
        shared.last_hr.store(i64::from(extract_hr(&e)), Ordering::Relaxed);
        shared.state.store(state_to_u8(StreamState::Failed), Ordering::Release);
        diag.record_device_error();
        return owned;
    }
    shared.state.store(state_to_u8(StreamState::Running), Ordering::Release);

    // The handle is stream-owned (created at open, closed in the stream's Drop *after* this
    // thread is joined), so using it here for this thread's whole lifetime is sound.
    let event = shared.event.load(Ordering::Relaxed) as HANDLE;
    let timeout_ms = (u32::try_from(period.as_millis()).unwrap_or(u32::MAX) * 4).max(250);
    let mut last_wake = Instant::now();

    while !shared.stop.load(Ordering::Acquire) {
        // SAFETY: valid owned event handle; bounded timeout so a wedged driver cannot hang
        // stop() forever — and stop() signals the event directly anyway.
        let wait = unsafe { WaitForSingleObject(event, timeout_ms) };
        let wake = Instant::now();
        diag.record_wake(wake - last_wake, period);
        last_wake = wake;

        if shared.stop.load(Ordering::Acquire) {
            break;
        }
        if wait == WAIT_TIMEOUT {
            // No device event for 4 periods: the engine stalled (driver wedged, session killed).
            // record_wake already counted the late wake/xrun; keep waiting — stop() still wakes
            // us via SetEvent, and a recovering driver resumes signalling.
            continue;
        }
        if wait != WAIT_OBJECT_0 {
            // WAIT_FAILED: the wait itself is broken; fail the stream rather than spin.
            shared.last_hr.store(i64::from(WIN32_ERROR_SENTINEL), Ordering::Relaxed);
            shared.state.store(state_to_u8(StreamState::Failed), Ordering::Release);
            diag.record_device_error();
            break;
        }

        // Write one device period (minus padding) from the FIFO. All raw-pointer discipline
        // lives inside write_from_fifo; the pointer never reaches this scope.
        let write = (|| -> Result<u64, HalError> {
            let padding = owned.bundle.client.current_padding()? as usize;
            let writable = period_frames.saturating_sub(padding);
            if writable == 0 {
                return Ok(0);
            }
            let got = owned.bundle.render.write_from_fifo(
                writable as u32,
                ch,
                neg.fmt,
                &mut owned.fifo,
            )?;
            if got < writable * ch {
                // FIFO starvation: the remainder was silence (pop_into_* zero-filled) and the
                // glitch is counted — audible truth, counted truth. The device still ACCEPTED
                // the full `writable`, and the drift metric says so: two truths, two counters,
                // neither dressed as the other.
                diag.record_xrun();
            }
            Ok(writable as u64)
        })();
        match write {
            // Drift against wall time, re-derived per #76: frames the device ACCEPTED vs the
            // frames the negotiated rate promises. A healthy stream sits near 0 ppm in BOTH
            // share modes; the old GetPosition derivation read +1.2 M ppm on a soaked-clean
            // shared stream (§4b) and hid a real half-throughput exclusive inside the same
            // SUSPECT label (attempt 3, §4e) — one label, two opposite stories, no verdict.
            Ok(released) => {
                delivered_frames += released;
                diag.record_device_clock(delivered_frames, start.elapsed());
            },
            Err(e) => {
                diag.record_device_error();
                let hr = extract_hr(&e);
                shared.last_hr.store(i64::from(hr), Ordering::Relaxed);
                let removed = hr == AUDCLNT_E_DEVICE_INVALIDATED;
                shared.state.store(
                    state_to_u8(if removed { StreamState::Removed } else { StreamState::Failed }),
                    Ordering::Release,
                );
                break;
            },
        }

        // Refill: keep the FIFO at one period + one block of cushion. Bounded work per event:
        // at most ceil((period + block) / block) callbacks — which is what the budget tracks.
        let cushion = (period_frames + block) * ch;
        while owned.fifo.len() < cushion && owned.fifo.free() >= block * ch {
            run_block!();
        }
    }

    owned.bundle.client.stop();
    if state_from_u8(shared.state.load(Ordering::Acquire)) == StreamState::Running {
        shared.state.store(state_to_u8(StreamState::Stopped), Ordering::Release);
    }
    drop(rt); // revert MMCSS on this thread before COM goes away
    drop(com);
    owned
}

/// WAIT_FAILED carries its detail in GetLastError, not in an HRESULT; store a distinct sentinel
/// so the report says "wait failed" instead of impersonating a driver code.
const WIN32_ERROR_SENTINEL: i32 = -1;

/// Pull the raw HRESULT back out of a classified error for `last_hr` (0 when it was not a
/// device HRESULT — e.g. a System error — so the report falls back to its text).
fn extract_hr(e: &HalError) -> HRESULT {
    let text = e.to_string();
    // classify() embeds "(HRESULT 0x........)" or the known-code name; parse the hex back.
    if let Some(i) = text.rfind("0x") {
        let hex: String = text[i + 2..].chars().take_while(|c| c.is_ascii_hexdigit()).collect();
        if let Ok(v) = u32::from_str_radix(&hex, 16) {
            return v as HRESULT;
        }
    }
    0
}

// --------------------------------------------------------------------------- tests

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_maps_hresult_to_recovery_strategy() {
        assert!(matches!(classify("x", AUDCLNT_E_DEVICE_INVALIDATED), HalError::Removed(_)));
        assert!(matches!(classify("x", AUDCLNT_E_EXCLUSIVE_MODE_NOT_ALLOWED), HalError::Busy(_)));
        assert!(matches!(classify("x", AUDCLNT_E_DEVICE_IN_USE), HalError::Busy(_)));
        assert!(matches!(classify("x", AUDCLNT_E_UNSUPPORTED_FORMAT), HalError::Format(_)));
        // the code name and hex survive into the text — logs must be searchable by both
        let m = classify("Initialize", AUDCLNT_E_DEVICE_INVALIDATED).to_string();
        assert!(m.contains("AUDCLNT_E_DEVICE_INVALIDATED"), "{m}");
        assert!(m.to_lowercase().contains("0x8889"), "{m}");
        assert!(m.contains("Initialize"), "{m}");
    }

    #[test]
    fn extract_hr_roundtrips_through_classify() {
        // The pump stores last_hr by extracting the code from the classified error; the stream
        // later re-classifies it. The round trip must be lossless for device errors.
        for hr in
            [AUDCLNT_E_DEVICE_INVALIDATED, AUDCLNT_E_DEVICE_IN_USE, AUDCLNT_E_UNSUPPORTED_FORMAT]
        {
            let e = classify("test", hr);
            assert_eq!(extract_hr(&e), hr, "round trip lost the code: {e}");
        }
    }

    #[test]
    fn fifo_roundtrips_and_zero_fills_on_underrun() {
        let mut f = Fifo::new(64);
        f.push(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(f.len(), 4);
        let mut out = [0.0f32; 8];
        let got = f.pop_into_f32(&mut out);
        assert_eq!(got, 4);
        assert_eq!(&out[..4], &[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(&out[4..], &[0.0; 4], "the underrun tail is silence, not garbage");
        assert_eq!(f.len(), 0);
    }

    #[test]
    fn fifo_wraps_without_losing_order() {
        let mut f = Fifo::new(64); // 64 slots
        for round in 0..4 {
            let src: Vec<f32> = (0..40).map(|i| (round * 40 + i) as f32).collect();
            f.push(&src);
            let mut out = vec![0.0f32; 40];
            assert_eq!(f.pop_into_f32(&mut out), 40);
            assert_eq!(out, src, "round {round} came back wrong (wrap bug)");
        }
    }

    #[test]
    fn fifo_i16_conversion_clamps_and_stays_symmetric() {
        let mut f = Fifo::new(16);
        f.push(&[1.5, -1.5, 0.5, -0.5, 0.0]);
        let mut out = [0i16; 5];
        f.pop_into_i16(&mut out);
        assert_eq!(out[0], 32767, "above full scale clamps, never wraps");
        assert_eq!(out[1], -32767, "and symmetrically below");
        assert!((out[2] - 16383).abs() <= 1);
        assert!((out[3] + 16383).abs() <= 1);
        assert_eq!(out[4], 0);
    }

    #[test]
    fn fifo_i32_conversion_saturates_and_zero_fills() {
        let mut f = Fifo::new(16);
        f.push(&[1.5, -1.5, 1.0, -1.0, 0.5, 0.0]);
        let mut out = [0i32; 8];
        let got = f.pop_into_i32(&mut out);
        assert_eq!(got, 6);
        assert_eq!(out[0], i32::MAX, "above full scale saturates, never wraps");
        assert_eq!(out[1], i32::MIN, "and symmetrically below");
        assert_eq!(out[2], i32::MAX, "+1.0 saturates to MAX — the documented one-LSB asymmetry");
        assert_eq!(out[3], i32::MIN, "-1.0 maps to MIN exactly");
        assert!((out[4] - 1_073_741_824).abs() <= 64, "0.5 → half scale");
        assert_eq!(out[5], 0);
        assert_eq!(&out[6..], &[0, 0], "the underrun tail is silence, not garbage");
    }

    #[test]
    fn fifo_overfill_drops_newest_without_panic() {
        let mut f = Fifo::new(64); // slots round to 64
        let big = vec![0.25f32; 1000];
        f.push(&big);
        assert!(f.len() <= 64);
        assert_eq!(f.free(), 0);
    }

    #[test]
    fn wfxe_int_math_is_consistent() {
        // Packed-struct rule: copy fields to locals before comparing (assert_eq! takes
        // references, and referencing a packed field is UB).
        let w = wfxe_int(96_000, 2, MASK_STEREO, 32, 24);
        let (align, avg) = (w.Format.nBlockAlign, w.Format.nAvgBytesPerSec);
        let (bits, cb) = (w.Format.wBitsPerSample, w.Format.cbSize);
        assert_eq!(align, 8, "2 ch x 32-bit container = 8 bytes per frame");
        assert_eq!(avg, 96_000 * 8);
        assert_eq!(bits, 32, "the container width, not the valid width");
        assert_eq!(cb, 22);
        let sub = w.SubFormat;
        assert!(guid_eq(&sub, &SUBTYPE_PCM), "integer rungs are SUBTYPE_PCM");
        let sixteen = wfxe_int(48_000, 1, MASK_STEREO, 16, 16);
        let (align16, avg16) = (sixteen.Format.nBlockAlign, sixteen.Format.nAvgBytesPerSec);
        assert_eq!(align16, 2);
        assert_eq!(avg16, 48_000 * 2);
        let mask16 = sixteen.dwChannelMask;
        assert_eq!(mask16, MASK_MONO, "mono overrides a stale mask");
    }

    #[test]
    fn merge_rates_keeps_discrete_rates_honest() {
        let r = merge_rates(&[44_100, 48_000, 96_000]);
        assert_eq!(r.len(), 3, "audio rates are discrete; they must not merge into fake intervals");
        assert!(r.iter().all(|x| x.min == x.max));
    }

    #[test]
    fn wfx_f32_math_is_consistent() {
        // WAVEFORMATEX(TENSIBLE) are packed(1): copy fields to locals before comparing
        // (assert_eq! takes references, and referencing a packed field is UB).
        let w = wfx_f32(96_000, 8);
        let (align, avg) = (w.nBlockAlign, w.nAvgBytesPerSec);
        assert_eq!(align, 32);
        assert_eq!(avg, 96_000 * 32);
        let e = wfxe_f32(48_000, 2, MASK_STEREO);
        let (cb, sub, mask) = (e.Format.cbSize, e.SubFormat, e.dwChannelMask);
        assert_eq!(cb, 22);
        assert!(guid_eq(&sub, &SUBTYPE_IEEE_FLOAT));
        assert_eq!(mask, MASK_STEREO);
        let mono = wfxe_f32(48_000, 1, MASK_STEREO);
        let mono_mask = mono.dwChannelMask;
        assert_eq!(mono_mask, MASK_MONO, "mono overrides a stale mask");
        let eight = wfxe_f32(48_000, 8, 0xFF);
        let eight_mask = eight.dwChannelMask;
        assert_eq!(eight_mask, 0xFF, "multichannel keeps the device's own mask");
    }

    #[test]
    fn guid_constants_match_the_published_sdk_values() {
        // A typo in a GUID would fail at CoCreateInstance with REGDB_E_CLASSNOTREG on every
        // machine — the split-field spelling below is checked independently of from_u128.
        assert_eq!(CLSID_MMDEVICE_ENUMERATOR.data1, 0xBCDE_0395);
        assert_eq!(
            CLSID_MMDEVICE_ENUMERATOR.data4,
            [0x8E, 0x3D, 0xC4, 0x57, 0x92, 0x91, 0x69, 0x2E]
        );
        assert_eq!(IID_IAUDIO_CLIENT.data4[7], 0xB2);
        assert_eq!(IID_IAUDIO_RENDER_CLIENT.data1, 0xF294_ACFC);
        assert_eq!(SUBTYPE_IEEE_FLOAT.data3, 0x0010);
        assert_eq!(SUBTYPE_PCM.data1, 1);
        assert_eq!(PKEY_DEVICE_FRIENDLY_NAME.pid, 14);
        assert!(guid_eq(&SUBTYPE_IEEE_FLOAT, &guid(0x00000003_0000_0010_8000_00AA00389B71)));
        assert!(!guid_eq(&SUBTYPE_IEEE_FLOAT, &SUBTYPE_PCM));
    }
}
