# SUCCESS! SSH Enabled on IMC via 603E

**Дата:** 2026-02-05
**Результат:** SSH включен!

---

## РАБОЧАЯ ПОСЛЕДОВАТЕЛЬНОСТЬ

```
1. Extended Session (10 03)
   TX: 10 03
   RX: 50 03 00 32 01 F4  ← OK

2. Security Level 0x11 - Request Seed
   TX: 27 11
   RX: 67 11 XX XX XX  ← Seed

3. Security Level 0x11 - Send Key
   TX: 27 12 <key>
   RX: 67 12  ← UNLOCKED

4. Routine 603E (SSH Enable)
   TX: 31 01 60 3E 01
   RX: 7F 31 78  ← Pending...
   RX: 71 01 60 3E 22  ← SUCCESS!
```

---

## КЛЮЧЕВЫЕ НАХОДКИ

### 1. Extended Session работает ТОЛЬКО на реальной машине
- На бенче: NRC 0x12 (SubFunctionNotSupported)
- На машине: 50 03 = OK
- Причина: IMC проверяет наличие BCM/других ECU на CAN

### 2. Security Level
- **Level 0x09** - НЕ существует (даже на машине NRC 0x12)
- **Level 0x11** - РАБОТАЕТ в Extended Session!
- **Level 0x01** - только для Programming Session

### 3. FixedData для Level 0x11
```python
DC0314 = [0x65, 0xF8, 0x24, 0xAC, 0x8F]
```

### 4. Формат 603E
```
31 01 60 3E 01  ← нужен байт параметра!
```
- `31 01 60 3E` без параметра → NRC 0x13 (IncorrectMsgLength)
- `31 01 60 3E 00` или `01` → NRC 0x78 (Pending) → 71 01 60 3E 22 (Success)

### 5. NRC 0x78 = Response Pending
После отправки 603E нужно ждать ~1-2 сек для завершения

---

## АЛГОРИТМ: Ford KeyGenMkI

```python
def KeyGenMkI(seed_int, sknum, sknum2, sknum3, sknum4, sknum5):
    sknum13 = (seed_int >> 0x10) & 0xFF
    b2 = (seed_int >> 8) & 0xFF
    b3 = seed_int & 0xFF
    sknum6 = (sknum13 << 0x10) + (b2 << 8) + b3
    sknum7 = ((sknum6 & 0xFF0000) >> 0x10) | (sknum6 & 0xFF00) | (sknum << 0x18) | ((sknum6 & 0xFF) << 0x10)
    sknum8 = 0xC541A9  # Initial constant

    for i in range(0x20):
        sknum10 = (((sknum7 >> i & 1) ^ (sknum8 & 1)) << 0x17) | (sknum8 >> 1)
        old = sknum8 >> 1
        hb = (sknum10 & 0x800000) >> 0x17
        sknum8 = ((sknum10 & 0xEF6FD7) |
                  (((sknum10 & 0x100000) >> 0x14 ^ hb) << 0x14) |
                  (((old & 0x8000) >> 0xF ^ hb) << 0xF) |
                  (((old & 0x1000) >> 0xC ^ hb) << 0xC) |
                  (0x20 * ((old & 0x20) >> 5 ^ hb)) |
                  (8 * ((old & 8) >> 3 ^ hb)))

    fc = (sknum5 << 0x18) | (sknum4 << 0x10) | sknum2 | (sknum3 << 8)

    for j in range(0x20):
        sknum12 = (((fc >> j & 1) ^ (sknum8 & 1)) << 0x17) | (sknum8 >> 1)
        old = sknum8 >> 1
        hb = (sknum12 & 0x800000) >> 0x17
        sknum8 = ((sknum12 & 0xEF6FD7) |
                  (((sknum12 & 0x100000) >> 0x14 ^ hb) << 0x14) |
                  (((old & 0x8000) >> 0xF ^ hb) << 0xF) |
                  (((old & 0x1000) >> 0xC ^ hb) << 0xC) |
                  (0x20 * ((old & 0x20) >> 5 ^ hb)) |
                  (8 * ((old & 8) >> 3 ^ hb)))

    r = (((sknum8 & 0xF0000) >> 0x10) | (0x10 * (sknum8 & 0xF)) |
         ((((sknum8 & 0xF00000) >> 0x14) | ((sknum8 & 0xF000) >> 8)) << 8) |
         (((sknum8 & 0xFF0) >> 4) << 0x10))
    return r & 0xFFFFFF
```

---

## ФАЙЛЫ

### Рабочие скрипты (Windows Desktop):
- `try_603e_wait.py` - **РАБОЧИЙ СКРИПТ для 603E**
- `level11_603e.py` - Level 0x11 + routines
- `sniff.py` - CAN sniffer

### CAN дампы:
- `C:\Users\Andrei\Desktop\can_124228.txt` - 142K сообщений (машина заведена)
- `C:\Users\Andrei\Desktop\ign_124517.txt` - 35K сообщений (зажигание вкл/выкл)
- `C:\Users\Andrei\Desktop\long_125249.txt` - 221K сообщений (IGN ON/OFF cycle)

### На Mac:
- `/Users/andrei/jlr/SUCCESS_603E.md` - этот файл
- `/Users/andrei/jlr/BENCH_EMULATION.md` - план эмуляции на бенче
- `/Users/andrei/jlr/can_dump_car.txt` - копия CAN дампа (142K)
- `/Users/andrei/jlr/long_dump_ign_cycle.txt` - длинный дамп с IGN cycle (221K)
- `/Users/andrei/jlr/scripts/try_603e_wait.py` - **РАБОЧИЙ СКРИПТ**

---

## ПОДКЛЮЧЕНИЕ К IMC

После успешного 603E:
1. **Mac USB-C -> USB-A адаптер -> USB порт в подлокотнике** (напрямую, без Ethernet!)
2. Mac должен увидеть новый сетевой интерфейс (USB Gadget mode)
3. IP: `192.168.103.11`
4. SSH: `ssh root@192.168.103.11`
5. **Password: `jlr`**

**ВАЖНО**: IMC представляется как USB сетевая карта (CDC-ECM/RNDIS), НЕ как Ethernet!
Никаких Ethernet кабелей не нужно - только USB напрямую в порт подлокотника.

---

## ПОЧЕМУ НЕ РАБОТАЛО НА БЕНЧЕ

IMC проверяет:
1. Наличие BCM на CAN шине (TX: 726, RX: 72E)
2. Ignition status от BCM
3. Другие ECU (IPC, TCM и т.д.)

Без этих ECU Extended Session блокируется (NRC 0x12).

---

## ЭМУЛЯЦИЯ НА БЕНЧЕ (TODO)

Для работы на бенче нужно эмулировать:
1. BCM heartbeat messages
2. Ignition ON status

CAN IDs для эмуляции - нужно проанализировать can_dump файлы.

---

## HISTORY

- 2026-02-04: Нашли Level 0x01 работает в Programming Session
- 2026-02-04: Реверс VISO14229.dll - SetParameter 0xE это НЕ UDS
- 2026-02-05: Тест на машине - Extended Session РАБОТАЕТ!
- 2026-02-05: Нашли Level 0x11 работает в Extended Session
- 2026-02-05: **603E SUCCESS - SSH ENABLED!**
