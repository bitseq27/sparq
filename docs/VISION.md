# sparq — vision

*One page. Re-read this when the build feels pointless or the feature list feels endless.*

---

## What sparq is

A real-time, touch-first **modular instrument** in which mathematical structures, geometry and live data streams are simultaneously the sound source, the control source and the visual output.

It is built for one person's music: techno, breakcore, experimental. It is not a product for a market.

## What sparq is not

A DAW. A plugin host with a mixer. A timeline. A randomiser. A generic synth.

## The seven pillars

1. **Real-time correctness above features.** If it risks an xrun, it waits or it moves off the audio thread.
2. **Deterministic and reproducible.** Same patch + same seeds + same inputs + same time ⇒ same samples.
3. **Everything is a typed stream.** Audio is a fast float stream; a heartbeat sensor is a slow one. No special cases.
4. **The module boundary is a contract.** Additive evolution only. Old patches load forever.
5. **The UI is data-ink.** Every pixel is a real signal or a real control. Scientific instrument, not skin.
6. **Touch first.** No modifier keys, no right-click, nothing hover-only, nothing smaller than a finger.
7. **Music ships every phase.** A phase without a track made in the current build is an unfinished phase.

## The four ideas that make it *this* instrument

* **The patch is the score.** Project + seeds + journal reproduce a performance bit-exactly. Nothing is lost when the set ends.
* **Mutation is a subsystem.** L-systems, cellular automata, chaos attractors, Markov, spectral-of-sequence, genetic, and geometry-as-score — composed into chains and a navigable **DNA tree**, so exploration is directed rather than mush.
* **Geometry is audible and visible.** Draw a function, hear it. Project a rhythm onto a shape; rotate the shape, rotate the music. The same structures drive the sound and the projected visual.
* **Analysis is a control source.** Every analyser output is a stream you can modulate with, sequence from, and display. Meters are a side effect.

## The sound it exists to make

Autechre's systems-that-compose-themselves and inharmonic metal. Venetian Snares' sliced breaks in impossible metres. Squarepusher's acoustic-synthetic collision and real harmony. Richard Devine's maximalist sound design and intricate generative arpeggiation. Ikeda's data-as-material and audiovisual unity. Plus whatever those influences can't predict — which is the actual point.

## The look it exists to have

Near-black ground. Hairline traces. Phosphor accents that mean *signal class*, never decoration. Tabular monospace numerals with SI units. A screenshot with no title should read as scientific software.

## The test that settles arguments

> **Would this make me want to play tonight?**

If a feature doesn't serve that, it goes in `LATER.md`. If two weeks pass where the answer is no, stop building and fix playability.

## The promise to the future maintainer (you, in 18 months)

Every significant decision has an ADR saying what was chosen, what was rejected, and why. Nothing here is arbitrary, and nothing here is sacred — but change it in writing.
