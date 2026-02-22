# UDS App — Project Context

## JLR SDD Reference Data

### SDD Program (local copy)
```
/Users/andrei/JLR_SDD/JLR/SDD/Runtime/   — DLLs (SecurityAccessSyscall.dll, VISO14229.dll, etc.)
/Users/andrei/JLR_SDD/JLR/IDS/Xml/       — Vehicle configs (EXML encrypted)
```

Thats your source of truth. Any security access, dids and so on are there.

### DID Sources — IMPORTANT
Model-specific MDX EXML files (e.g. `MDX_IMC.exml`) are **incomplete** — they only list DIDs the SDD software download process uses, NOT all DIDs the ECU supports.

The full list of standard ISO 14229 DIDs (0xF100-0xF1FF range) is in:
```
/Users/andrei/JLR_SDD/JLR/Common/SDD/Data/en/gradex/Snapshot/Unqualified DID Formatting.xml
```
This file contains ALL standard DIDs across all ECU types. When looking for readable DIDs, ALWAYS check this file first, not just the model MDX.

Example: DID F111 (ECU Internal Number / firmware part) is NOT in any MDX but works perfectly on bench. F190 (VIN) times out because it's not programmed on bench units.

### Decode / Encode EXML
Use exml_decrypt.py here in root proejct to decode any exmls.

### CAN dumps
Within a project you can find various can dumps taken from a real vehicle.

### Bench Mode Notes
- MongoosePro JLR only supports ONE CAN channel — CAN broadcast (NM messages) cannot be sent while ISO15765 diagnostic channel is open
- Security Access (0x27) is NOT needed for reading DIDs — only for routines. Attempting SA and getting NRC 0x37 disrupts ECU state
- After any DID read failure/timeout, re-establish Extended Session (TesterPresent + 10 03) before next read — timeouts burn the S3 session timer
- Stale NRC responses from previous requests can arrive late — always validate NRC service ID matches the request
- Some DIDs (F190 VIN, F18C ECU Serial) timeout on bench because data was never programmed into the ECU
- DID F111 (firmware part number) always works — it's burned at manufacture, no dependencies


### Connection to remote Windows Machine where UDS app is being tested
sshpass to 192.168.1.16 with username "Andrei" and password "123".
Folder under C:\udsapp 

### Changes
After each change, you muts commit adn dont forget to keep pulling latest on windows machine.

### Testing
Never write garbage tests that do not test real flow, application. I dont wanna 200 tests running that do nothing and app doesnt work becuase main flows are not tested at all.