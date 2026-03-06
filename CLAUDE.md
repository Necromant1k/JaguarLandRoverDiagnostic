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

## THE MAIN PROBLEM — IMC Variant Config Broken After 0x6038

### Symptoms
- Car: X260 Jaguar XF MY16, 2.0L GTDi, VIN SAJBL4BVXGCY16353
- IMC (InControl Touch Pro Gen2) shows **8" display layout** instead of correct **10"**
- **No Apple CarPlay**, no heated/ventilated seats, other features missing
- IMC is in "emergency mode" — all default/fallback values applied

### How It Happened
1. User ran SDD "InControl Touch Pro variant configuration" on the working car
2. SDD selected **IMC_ERASE** ("Erase and learn variant configuration") — the only option
3. SDD ran the full sequence: 0x0E08 → 0x0E06 → 0x6038 → ECU Reset → 90s wait
4. **ALL steps returned PASS** (0x6038 STATUS=0x10, RESULT=0x01, ERROR=0x00)
5. After reboot, IMC showed 8" layout, no CarPlay, no heated seats — **broken**
6. Re-running the same SDD sequence gives PASS again but config stays wrong

### Second IMC Block — Same Result
- User **bought a used IMC** with known correct working config
- Ran 0x6038 through SDD on the car → **same result: broke with identical symptoms**
- Now has **TWO broken IMC blocks**, both showing 8" defaults

### Root Cause Analysis (In Progress)
The IMC's `vc_config.json` (`/opt/jlr/etc/vconf/vc_config.json`) has:
- **Default: `DisplaySize=8`** (fallback when CCF option 467 is missing/unmatched)
- **CCF option 467 (eCCF467_FRNTDISPVARIANT):**
  - 0x02/0x03 → `DisplaySize=8` (8-inch)
  - **0x04/0x05 → `DisplaySize=10`** (10-inch, correct for this car)

**Theory**: During 0x0E08→0x0E06, the IMC requests CCF from GWM via CAN. If:
- (A) GWM CCF has wrong option 467 value → IMC correctly applies wrong config
- (B) CCF transfer fails silently → IMC falls back to defaults (8-inch)
Either way, the ERASE step wiped good config and the LEARN step applied wrong values.

**SDD .dbg log** (from `/Users/andrei/Downloads/Untitled (1).dbg`) confirms entire sequence passes on the real car. No errors in any step.

### What We Tried (All Failed)
- Running 0x6038 via SDD multiple times — PASS but config stays wrong
- Buying a second working IMC and running 0x6038 — also broke
- The `restore_ccf` command in the app (same sequence as SDD) — pointless, does the same thing

### What Needs To Be Done
1. **Read GWM CCF option 467** from the real car to check if it's correct (0x04/0x05)
2. If wrong → need to **write correct CCF value to GWM** (WriteDID 0x2E on DID 0xEE00)
3. If correct → investigate CAN transfer failure between GWM and IMC during 0x0E08/0x0E06
4. Consider: SSH into IMC via 0x603E routine to manually fix Linux config files

### Key Files in RFS3 Image
```
/opt/jlr/etc/vconf/vc_config.json          — CCF→boot param mapping (2.5MB)
/opt/jlr/etc/ci/ci.lcf                     — display layout (hardcoded 888x542 = 8")
/opt/jlr/bin/DM/NGI-DiagnosisManager       — handles UDS routines including 0x6038
/opt/jlr/share/graphics/2d/layout_*/        — display layout directories
/opt/jlr/share/2dgraphicsvariant           — symlink to active layout
```
RFS3 image location: `/Users/andrei/Documents/Backup IMC Full Current/RFS3/gitlab-hybrid-imc-x86_32-latest_extracted/`

### All CCF Options Used by IMC 0x6038 (from vc_config.json)
Options: 1 (VehicleType), 6 (Fuel), 8 (SteeringWheel), 10 (Gearbox), 49 (HeatedRearSeat),
54 (HeatedFrontSeats), 59 (ParkingAssist), 119 (RadioAmpSpeaker), 127 (Navigation),
157 (Bluetooth), 173 (RearEntertainment), 212 (DVDRegion), 449 (CameraHMI),
**467 (FrontDisplayVariant)**, 468 (FrontAVIOPanel), 623 (ClusterCableLength),
641 (FrontLower/UpperDisplayVariant), 642 (FrontUpperDeployable),
664 (LHSWSFavourite), 665 (IMCClusterAPIX)