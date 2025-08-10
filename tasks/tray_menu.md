# Voice Input Tray Menu UX Design

This document outlines the UX design for the Voice Input application's tray menu, addressing the requirements to:
1. Show the current model
2. Show which models are already downloaded
3. Allow users to choose a model
4. Show which models are loading right now
5. Show the current app state (recording, transcribing, loading the first model, ready for recording)

## Idle State (Ready for Recording)

```
┌─────────────────────────────┐
│ 🎙️ Voice Input              │
├─────────────────────────────┤
│ Status: Ready               │
│ Model: base (✓)             │
├─────────────────────────────┤
│ Change Model ▶              │
│ About                       │
│ Quit                        │
└─────────────────────────────┘
```

**Legend:**
- ✓ = Downloaded model
- ⬇ = Currently downloading
- ○ = Not selected model
- ● = Currently selected model

## Recording State

```
┌─────────────────────────────┐
│ 🔴 Voice Input              │
├─────────────────────────────┤
│ Status: Recording...        │
│ Duration: 00:05             │
│ Model: base (✓)             │
├─────────────────────────────┤
│ Change Model ▶              │
│ About                       │
│ Quit                        │
└─────────────────────────────┘
```

## Processing State (Transcribing)

```
┌─────────────────────────────┐
│ ⏳ Voice Input              │
├─────────────────────────────┤
│ Status: Transcribing...     │
│ Model: base (✓)             │
├─────────────────────────────┤
│ Change Model ▶              │
│ About                       │
│ Quit                        │
└─────────────────────────────┘
```

## Model Loading State (First Model)

```
┌─────────────────────────────┐
│ ⬇ Voice Input              │
├─────────────────────────────┤
│ Status: Loading first model │
│ Progress: 45%               │
│ Model: base (⬇)             │
├─────────────────────────────┤
│ Change Model ▶              │
│ About                       │
│ Quit                        │
└─────────────────────────────┘
```

## Model Loading State (Additional Model)

```
┌─────────────────────────────┐
│ 🎙️ Voice Input              │
├─────────────────────────────┤
│ Status: Ready               │
│ Model: small (✓)            │
│ Downloading: medium (67%)   │
├─────────────────────────────┤
│ Change Model ▶              │
│ About                       │
│ Quit                        │
└─────────────────────────────┘
```

## Model Selection Interface

```
┌─────────────────────────────┐
│ Select Model:               │
├─────────────────────────────┤
│ ● base (✓)                  │
│   Size: 142MB               │
│                             │
│ ○ small (✓)                 │
│   Size: 466MB               │
│                             │
│ ○ medium (not downloaded)   │
│   Size: 1.5GB               │
│   [Download]                │
│                             │
│ ○ large (not downloaded)    │
│   Size: 2.9GB               │
│   [Download]                │
└─────────────────────────────┘
```

## Error State (Model Download Failed)

```
┌─────────────────────────────┐
│ ⚠️ Voice Input              │
├─────────────────────────────┤
│ Status: Ready               │
│ Model: base (✓)             │
│ Failed: medium [Retry]      │
│ Error: Network timeout      │
├─────────────────────────────┤
│ Change Model ▶              │
│ About                       │
│ Quit                        │
└─────────────────────────────┘
```

Note: When at least one model is loaded, the app status shows the status of the loaded model (e.g., "Ready") rather than "Download failed". The download failure is shown as a separate line.

## Legend for All Views

- 🎙️ = Ready for recording
- 🔴 = Recording in progress
- ⏳ = Processing/Transcribing
- ⬇ = Downloading model
- ⚠️ = Error state
- ✓ = Downloaded model
- ⬇ = Currently downloading
- ❌ = Download failed
- ○ = Not selected model
- ● = Currently selected model