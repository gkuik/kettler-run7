# Kettler binary protocol (from the kdri crate). Handshake + READ instructions only.
import asyncio, sys, time
from bleak import BleakScanner, BleakClient
OUT = sys.argv[1] if len(sys.argv) > 1 else None
f = open(OUT, "w", buffering=1) if OUT else None
t0 = time.time()
def p(*a):
    a = (f"{time.time()-t0:6.2f}",) + a
    print(*a)
    if f: print(*a, file=f)

W = "0000fff2-0000-1000-8000-00805f9b34fb"; N = "0000fff1-0000-1000-8000-00805f9b34fb"
TABLE = []
for i in range(256):
    crc, d = 0, i
    for _ in range(8):
        crc = (crc >> 1) ^ 0x8408 if (d ^ crc) & 1 else crc >> 1
        d >>= 1
    TABLE.append(crc)
def crc16(bs):
    c = 0
    for b in bs: c = (c >> 8) ^ TABLE[(c ^ b) & 0xff]
    return c
def frame(payload):
    out = bytearray([0x02])
    for b in payload: out += bytes([0x10, b ^ 0x20]) if b in (2, 3, 0x10) else bytes([b])
    c = crc16(payload)
    return bytes(out + bytes([0x03, c >> 8, c & 0xff]))
HANDSHAKE = bytes([0x00,0x01,0x01,0x00,0x12,0x2d,0x24,0xf2,0x24,0x96,0xa4,0xff,0x98,0x29,0xf9,0x21,0xbe,0x9d,0xaa,0x9e,0x4d,0x01,0x17])
assert frame(HANDSHAKE)[-2:] == bytes([0x86, 0x77]), "CRC mismatch vs kdri reference"
READ = 1
def read(vid): return frame(bytes([vid >> 8, vid & 0xff, READ, 0, 0]))

NAMES = {6:"DeviceState",7:"BrakeMode",8:"Pulse",9:"Rpm",10:"PowerSet",11:"PowerGet",15:"Distance",16:"Energy",17:"Time",18:"TimeMode",
         20:"DeviceName",24:"DeviceType",25:"DeviceId",30:"InclineSet",31:"InclineGet",32:"InclineMin",33:"InclineMax",
         34:"SpeedSet",35:"SpeedGet",36:"SpeedMin",37:"SpeedMax",38:"Online"}
STATIC = [24, 20, 25, 36, 37, 32, 33]
DYNAMIC = [6, 35, 34, 31, 30, 15, 17, 16, 8, 18, 38]

class Parser:
    def __init__(s): s.reset()
    def reset(s): s.buf, s.esc, s.inframe, s.crc = bytearray(), False, False, None
    def feed(s, data):
        for b in data:
            if not s.inframe:
                if b == 0x02: s.reset(); s.inframe = True
                continue
            # CRC bytes are byte-stuffed like the payload
            if s.esc: b ^= 0x20; s.esc = False
            elif b == 0x10: s.esc = True; continue
            elif s.crc is None and b == 0x03: s.crc = bytearray(); continue
            if s.crc is not None:
                s.crc.append(b)
                if len(s.crc) == 2:
                    ok = crc16(s.buf) == (s.crc[0] << 8 | s.crc[1])
                    s.handle(bytes(s.buf), ok); s.reset()
            else: s.buf.append(b)
    def handle(s, pl, ok):
        if len(pl) < 5: p("short frame", pl.hex(" "), "crc ok" if ok else "BAD CRC"); return
        vid, ins, ln = pl[0] << 8 | pl[1], pl[2], pl[3] << 8 | pl[4]
        data = pl[5:5+ln]
        name = NAMES.get(vid, f"value#{vid}")
        if ln == 2: val = data[0] << 8 | data[1]
        elif name in ("DeviceName", "DeviceType", "DeviceId"): val = repr(bytes(data).decode("latin1"))
        elif ln == 1: val = data[0]
        else: val = data.hex(" ")
        p(f"  ANSWER {name:12} ins={ins} len={ln} -> {val}", "" if ok else "(BAD CRC)")

# --- Guided observation test. READ-ONLY: never writes anything except the handshake.
# Phase 1: log while the user walks at a fixed speed (units of Distance/Energy).
# Phase 2: drop the BLE link while the belt runs (does the belt stop?).
# Phase 3: reconnect and log while the user stops the belt and pulls the safety key (DeviceState).
import subprocess
LOG_SECONDS, GAP_SECONDS, PHASE3_SECONDS = 180, 25, 75
WATCH = (35, 34, 15, 16, 38, 6, 31, 8)
state = {}
_orig = Parser.handle
def handle(s, pl, ok):
    if ok and len(pl) >= 5:
        ln = pl[3] << 8 | pl[4]
        state[pl[0] << 8 | pl[1]] = int.from_bytes(pl[5:5+ln], "big") if 0 < ln <= 2 else None
Parser.handle = handle

def say(text):
    p("SAY", text)
    subprocess.Popen(["say", "-v", "Thomas", text], stderr=subprocess.DEVNULL)

def row(tag):
    p(f"{tag} speed={state.get(35)} set={state.get(34)} dist={state.get(15)} energy={state.get(16)} online={state.get(38)} devstate={state.get(6)} incl={state.get(31)} pulse={state.get(8)}")

async def session(d, seconds, tag, wait_running=False):
    parser = Parser()
    async with BleakClient(d, disconnected_callback=lambda c: p("DISCONNECTED")) as c:
        p("connected")
        async def send(b):
            for i in range(0, len(b), 20):
                await c.write_gatt_char(W, b[i:i+20], response=False)
        async def poll_once():
            for vid in WATCH:
                await send(read(vid)); await asyncio.sleep(0.08)
            await asyncio.sleep(0.3)
        await c.start_notify(N, lambda h, b: parser.feed(b))
        await send(frame(HANDSHAKE)); await asyncio.sleep(1)
        if wait_running:
            state.clear()
            say("Connecté. Démarrez le tapis depuis la console, réglez trois kilomètres heure, puis marchez sans toucher aux réglages.")
            end = time.time() + 120
            while time.time() < end and not state.get(35):
                await poll_once(); row("wait")
            if not state.get(35): say("Le tapis ne tourne pas. Test annulé."); return False
            say("C'est parti pour trois minutes d'enregistrement.")
        start = time.time(); dist_int = 0.0; last = time.time(); announced = set()
        while time.time() - start < seconds and c.is_connected:
            await poll_once()
            now = time.time(); dist_int += (state.get(35) or 0) / 10 / 3.6 * (now - last); last = now
            row(f"{tag} t={now-start:5.1f} integ_m={dist_int:6.1f}")
            left = int(seconds - (now - start))
            if tag == "LOG" and left <= 60 and 60 not in announced: announced.add(60); say("Encore une minute.")
            if tag == "AFTER" and 0 not in announced:
                announced.add(0)
                say("Reconnecté. Arrêtez le tapis depuis la console. Notez la distance et les calories affichées. Puis retirez la clé de sécurité cinq secondes, et remettez-la.")
        p(f"== {tag} end: integrated distance from SpeedGet = {dist_int:.1f} m over {time.time()-start:.0f} s")
        if tag == "LOG": say("Je coupe le Bluetooth maintenant. Continuez à marcher, et regardez si le tapis s'arrête.")
        return True

async def main():
    p("Scanning...")
    d = await BleakScanner.find_device_by_filter(lambda d,a: (a.local_name or d.name or "").replace(" ","").lower()=="run7", timeout=25)
    if not d: p("NOT FOUND"); say("Tapis introuvable."); return
    if not await session(d, LOG_SECONDS, "LOG", wait_running=True): return
    p(f"== link dropped at {time.time()-t0:.1f}, waiting {GAP_SECONDS} s")
    await asyncio.sleep(GAP_SECONDS)
    say("Je me reconnecte.")
    d = await BleakScanner.find_device_by_filter(lambda d,a: (a.local_name or d.name or "").replace(" ","").lower()=="run7", timeout=25)
    if not d: p("NOT FOUND on reconnect"); say("Reconnexion impossible. Arrêtez le tapis depuis la console."); return
    await session(d, PHASE3_SECONDS, "AFTER")
    say("Test terminé. Merci.")
    p("DONE")
try: asyncio.run(main())
except Exception as e: p("CRASH", repr(e)); subprocess.run(["say", "Erreur. Arrêtez le tapis depuis la console."])
p("END")
