# UDS App — Project Context

## JLR SDD Reference Data

### SDD Program (local copy)
```
/Users/andrei/JLR_SDD/JLR/SDD/Runtime/   — DLLs (SecurityAccessSyscall.dll, VISO14229.dll, etc.)
/Users/andrei/JLR_SDD/JLR/IDS/Xml/       — Vehicle configs (EXML encrypted)
```

### Decrypted EXML / Research
```
/Users/andrei/jlr/                        — Main research folder
/Users/andrei/jlr/CLAUDE.md              — Full research notes (MUST READ for IMC/security context)
/Users/andrei/jlr/exml_decrypt.py        — EXML decryptor (3DES ECB)
/Users/andrei/jlr/IMC_decrypted.xml      — Decrypted IMC config
/Users/andrei/jlr/BCM_decrypted.xml      — Decrypted BCM config
/Users/andrei/jlr/MDX_IMC_decrypted.xml  — Decrypted MDX (routines, DIDs)
```

### CAN Dumps
```
/Users/andrei/jlr/can_dump_ign_onoff.txt      — Ignition on/off cycle (bench)
/Users/andrei/jlr/can_dump_car.txt             — Real car CAN traffic
/Users/andrei/jlr/can_dump_with_sdd.txt        — CAN during SDD session
/Users/andrei/jlr/can_dump_ignition.txt        — Ignition sequence
/Users/andrei/jlr/long_dump_ign_cycle.txt      — Long ignition cycle dump
/Users/andrei/jlr/can_full_dump.txt            — Full raw CAN dump
```

### SDD Debug Logs
```
/Users/andrei/Downloads/forceign.dbg           — BCM auto-ignition flow
/Users/andrei/Downloads/603d.dbg               — Successful 603D routine
/Users/andrei/Downloads/sharedsecret.dbg       — Shared secret flow
/Users/andrei/Downloads/failed.dbg             — Failed operation log
```

## IMC Bench Limitations

**CRITICAL**: On bench (without BCM/GWM on CAN bus):
- Extended Session (10 03) → NRC 0x12 (SubFunctionNotSupported)
- Programming Session (10 02) → Works, Level 0x01 unlock works
- Many DIDs return NRC 0x31 or timeout in default session
- Routines 603D/603E require Extended Session (blocked on bench)

## IMC Protocol (from decrypted EXML)

- TX: 0x7B3, RX: 0x7BB, 500kbps, 11-bit CAN
- Max 1 DID per ReadDID request (`ISO14229_SERVICE_22_MAX_DATA_ID=1`)
- RX timeout: 35s for engineering, 2s for TX
- Max retry period: 6s, max pending period: 60s
- Max busy attempts: 6

## SDD Auto-Ignition (BCM)
- Routine 0x2038, start sub_func=0x01, param=0x01
- BCM at 0x726/0x72E, 500kbps, 11-bit CAN
