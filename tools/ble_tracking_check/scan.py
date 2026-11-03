"""Scan for MeshPay BLE advertisements and write one JSONL window file."""
import argparse
import asyncio
import json
import sys
import time

from bleak import BleakScanner

# packages/meshpay_native/lib/meshpay_uuids.dart: MeshpayUuids.appServiceUuid
SERVICE_UUID = "0e4167c4-bc16-418c-b435-0e3340ea94ad"


def matches(adv):
  return SERVICE_UUID in [u.lower() for u in adv.service_uuids] or SERVICE_UUID in [
    u.lower() for u in adv.service_data
  ]


def record(device, adv):
  data = {u.lower(): v for u, v in adv.service_data.items()}
  return {
    "ts": time.time(),
    "rssi": adv.rssi,
    "peripheral_id": device.address,
    "service_data": data.get(SERVICE_UUID, b"").hex() or None,
    "name": adv.local_name,
    "manufacturer_data": {str(k): v.hex() for k, v in adv.manufacturer_data.items()} or None,
    "tx_power": adv.tx_power,
  }


async def main():
  parser = argparse.ArgumentParser()
  parser.add_argument("--seconds", type=int, default=60)
  parser.add_argument("--out", required=True, help="JSONL output, one file per window")
  args = parser.parse_args()
  count = 0
  with open(args.out, "w") as out:

    def on_adv(device, adv):
      nonlocal count
      if matches(adv):
        out.write(json.dumps(record(device, adv)) + "\n")
        out.flush()
        count += 1

    try:
      async with BleakScanner(on_adv):
        await asyncio.sleep(args.seconds)
    except Exception as error:
      print(f"SCAN FAILED: {type(error).__name__}: {error}", file=sys.stderr)
      return 2
  print(f"{count} MeshPay advertisements -> {args.out}")
  return 0


sys.exit(asyncio.run(main()))
