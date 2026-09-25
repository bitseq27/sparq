@echo off
color 0A
title GitHub Update Script

echo ========================================
echo       GitHub Project Update Script
echo ========================================
echo.

:: 1. Check if Git is installed
git --version >nul 2>&1
if %errorlevel% neq 0 (
    color 0C
    echo [ERROR] Git is not installed or not added to your system PATH.
    pause
    exit /b 1
)

:: 2. Check if the current directory is a Git repository
git rev-parse --is-inside-work-tree >nul 2>&1
if %errorlevel% neq 0 (
    color 0C
    echo [ERROR] This folder is not a Git repository.
    echo Please run this script from the root of your project.
    pause
    exit /b 1
)

:: 3. Check if there are any changes to commit
git diff-index --quiet HEAD --
if %errorlevel% equ 0 (
    echo [INFO] No changes detected. Working tree is clean.
    echo Checking for unpushed commits...
    
    :: Check if there are commits to push
    git log @{u}.. >nul 2>&1
    if %errorlevel% neq 0 (
        echo [INFO] Nothing to push. Everything is up to date!
        pause
        exit /b 0
    )
)

:: 4. Prompt for commit message
set /p commit_msg="Enter your commit message: "

if "%commit_msg%"=="" (
    color 0C
    echo [ERROR] Commit message cannot be empty. Aborting.
    pause
    exit /b 1
)

echo.
echo [1/3] Staging all changes...
git add .
if %errorlevel% neq 0 (
    color 0C
    echo [ERROR] Failed to stage files.
    pause
    exit /b 1
)

echo [2/3] Committing changes...
git commit -m "%commit_msg%"
if %errorlevel% neq 0 (
    color 0C
    echo [ERROR] Commit failed. Please check your git status.
    pause
    exit /b 1
)

echo [3/3] Pushing to GitHub...
git push
if %errorlevel% neq 0 (
    color 0C
    echo [ERROR] Push failed. Check your internet connection or GitHub credentials.
    pause
    exit /b 1
)

echo.
color 0A
echo ========================================
echo       Update Successfully Pushed!
echo ========================================
pause