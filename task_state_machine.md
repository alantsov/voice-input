implement state machine. it should be in the separate thread, not the main one.
it should receive only high-level events, like `start recording` but not low level like `ctrl key down`
should not use mutex at all - all variables should be thread local
thread with state machine should never perform heavy tasks, only delegate them to other threads

States would be 
1. loading initial model 
2. ready
3. recording
4. transcribing 

accepting events
1. model loaded, model_loading_failed
2. start recording, change model, load model, model loaded, model_loading_failed
3. stop recording, recording_stopped_by_os, recording_was_not_started
4. transcription finished, transcription failed

