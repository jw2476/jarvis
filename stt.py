import whisper
import sys
import torch

model = whisper.load_model("base.en")
options = whisper.DecodingOptions(language="english")

for line in sys.stdin: 
    file = line[:-1]
    print(f"Running on `{file}`", file=sys.stderr)
    audio = whisper.load_audio("output.wav")
    audio = whisper.pad_or_trim(audio)

    mel = whisper.log_mel_spectrogram(audio).to(model.device)
    result = whisper.decode(model, mel, options)
    print(result.text)
    sys.stdout.flush()
    print(result.text, file=sys.stderr)
    break

