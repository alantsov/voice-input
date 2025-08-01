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
│ 🎙️ Voice Input - Ready      │
├─────────────────────────────┤
│ ● Model: base ▼             │
│   ├─ ✓ base                 │
│   ├─ ✓ small                │
│   ├─ ⬇ medium (downloading) │
│   └─ ○ large                │
├─────────────────────────────┤
│ Status: Ready for recording │
├─────────────────────────────┤
│ About                       │
│ Quit                        │
└─────────────────────────────┘
```

**Legend:**
- ✓ = Downloaded model
- ⬇ = Currently downloading
- ○ = Not downloaded
- ● = Currently selected model

## Recording State

```
┌─────────────────────────────┐
│ 🔴 Voice Input - Recording   │
├─────────────────────────────┤
│ ● Model: base ▼             │
│   ├─ ✓ base                 │
│   ├─ ✓ small                │
│   ├─ ⬇ medium (downloading) │
│   └─ ○ large                │
├─────────────────────────────┤
│ Status: Recording...        │
│ Duration: 00:05             │
├─────────────────────────────┤
│ About                       │
│ Quit                        │
└─────────────────────────────┘
```

## Processing State (Transcribing)

```
┌─────────────────────────────┐
│ ⏳ Voice Input - Processing  │
├─────────────────────────────┤
│ ● Model: base ▼             │
│   ├─ ✓ base                 │
│   ├─ ✓ small                │
│   ├─ ⬇ medium (downloading) │
│   └─ ○ large                │
├─────────────────────────────┤
│ Status: Transcribing...     │
├─────────────────────────────┤
│ About                       │
│ Quit                        │
└─────────────────────────────┘
```

## Model Loading State (First Model)

```
┌─────────────────────────────┐
│ ⬇ Voice Input - Loading     │
├─────────────────────────────┤
│ ● Model: base ▼             │
│   ├─ ⬇ base (downloading)   │
│   ├─ ○ small                │
│   ├─ ○ medium               │
│   └─ ○ large                │
├─────────────────────────────┤
│ Status: Loading first model │
│ Progress: 45%               │
├─────────────────────────────┤
│ About                       │
│ Quit                        │
└─────────────────────────────┘
```

## Model Loading State (Additional Model)

```
┌─────────────────────────────┐
│ 🎙️ Voice Input - Ready      │
├─────────────────────────────┤
│ ● Model: small ▼            │
│   ├─ ✓ base                 │
│   ├─ ✓ small                │
│   ├─ ⬇ medium (downloading) │
│   │  Progress: 67%          │
│   └─ ○ large                │
├─────────────────────────────┤
│ Status: Ready for recording │
├─────────────────────────────┤
│ About                       │
│ Quit                        │
└─────────────────────────────┘
```

## Model Selection Interface

```
┌─────────────────────────────┐
│ 🎙️ Voice Input - Ready      │
├─────────────────────────────┤
│ ● Model: base ▼             │
│   ├─ ✓ base                 │
│   │  Size: 142MB            │
│   │  Languages: English     │
│   │                         │
│   ├─ ✓ small                │
│   │  Size: 466MB            │
│   │  Languages: English     │
│   │                         │
│   ├─ ○ medium               │
│   │  Size: 1.5GB            │
│   │  Languages: English     │
│   │  [Download]             │
│   │                         │
│   └─ ○ large                │
│      Size: 2.9GB            │
│      Languages: Multilingual│
│      [Download]             │
├─────────────────────────────┤
│ Status: Ready for recording │
├─────────────────────────────┤
│ About                       │
│ Quit                        │
└─────────────────────────────┘
```

## Error State (Model Download Failed)

```
┌─────────────────────────────┐
│ ⚠️ Voice Input - Error       │
├─────────────────────────────┤
│ ● Model: base ▼             │
│   ├─ ✓ base                 │
│   ├─ ✓ small                │
│   ├─ ❌ medium (failed)      │
│   │  [Retry Download]       │
│   └─ ○ large                │
├─────────────────────────────┤
│ Status: Download failed     │
│ Error: Network timeout      │
├─────────────────────────────┤
│ About                       │
│ Quit                        │
└─────────────────────────────┘
```

## Compact View (Alternative)

For systems with limited space in the tray area, a more compact version could be used:

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

When "Change Model" is selected:

```
┌─────────────────────────────┐
│ Select Model:               │
├─────────────────────────────┤
│ ● base (✓)                  │
│ ○ small (✓)                 │
│ ○ medium (⬇ 67%)            │
│ ○ large (not downloaded)    │
└─────────────────────────────┘
```

## Legend for All Views

- 🎙️ = Ready for recording
- 🔴 = Recording in progress
- ⏳ = Processing/Transcribing
- ⬇ = Downloading model
- ⚠️ = Error state
- ✓ = Downloaded model
- ⬇ = Currently downloading
- ❌ = Download failed
- ○ = Not downloaded
- ● = Currently selected model