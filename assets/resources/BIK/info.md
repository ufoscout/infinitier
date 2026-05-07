Downloaded from: https://samples.ffmpeg.org/game-formats/bink (and the `bikb/`
subfolder for the `BIKb` samples).

| File              | Codec | Notes                                                    |
| ----------------- | ----- | -------------------------------------------------------- |
| `CEVIL2.BIK`      | BIKb  | BinkB, no audio                                          |
| `TESTING.BIK`     | BIKb  | BinkB, no audio                                          |
| `logo_legal.bik`  | BIKi  | Bink-v1, no audio                                        |
| `logo_lucas.bik`  | BIKi  | Bink-v1, DCT audio @ 44 000 Hz                           |
| `original.bik`    | BIKi  | Bink-v1, **RDFT** audio @ 44 100 Hz                      |
| `WOTC.mve`        | BIKi  | Bink-v1, DCT audio @ 22 050 Hz — IWD2 file with `.mve` ext |

`WOTC.mve` keeps the `.mve` extension because that's how IWD2 ships it on
disk (the engine doesn't care about the extension; everything is detected
from the `BIKi` magic).
