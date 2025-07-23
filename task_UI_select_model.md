add model selection to tray menu

options: base, small, medium, large

for base, small, medium: option will be converted to model name as `ggml-{option}.en.bin` and `ggml-{option}.bin`
for large: `ggml-{option}.bin`

only base model should be downloaded on startup

other models should be downloaded if needed (and keep) when user selects a corresponding option for the first time

during loading indicate it in tray menu by adding string `loading...`