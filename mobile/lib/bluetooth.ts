// mobile/lib/bluetooth.ts
import { BleManager, Device } from "react-native-ble-plx";
import { Buffer } from "buffer";

const manager = new BleManager();

const SERVICE_UUID = "FB94E026-23E5-4BD9-97D6-74F25D579393";
const WRITE_CHAR_UUID = "935450A0-FAC2-4B9E-82FF-13E499710728";

function log(...args: any[]) {
  const t = new Date().toISOString().split("T")[1].replace("Z", "");
  console.log("[BLE " + t + "]", ...args);
}

// scan but only for 4 seconds now
export async function scanForDevices(timeoutMs = 4000): Promise<Device[]> {
  const found: Record<string, Device> = {};

  return new Promise((resolve, reject) => {
    log("🔍 scan start (", timeoutMs, "ms )");

    manager.startDeviceScan([SERVICE_UUID], null, (error, device) => {
      if (error) {
        log("❌ scan error:", error);
        manager.stopDeviceScan();
        reject(error);
        return;
      }

      if (device && !found[device.id]) {
        log("📡", device.name ?? "Unnamed", device.id);
        found[device.id] = device;
      }
    });

    setTimeout(() => {
      manager.stopDeviceScan();
      const list = Object.values(found);
      log("✅ scan done. found", list.length);
      resolve(list);
    }, timeoutMs);
  });
}

export async function connectToDevice(device: Device): Promise<Device> {
  log("🔗 connecting to", device.name ?? device.id);
  // if connect hangs, you can also wrap this in a Promise.race
  const connected = await device.connect();
  const withServices = await connected.discoverAllServicesAndCharacteristics();
  log("✅ connected + discovered");
  return withServices;
}

export async function sendMessageToDevice(device: Device, msg: string): Promise<void> {
  // firmware expects UTF-8, so we **must** send base64-encoded utf8
// const base64Msg = Buffer.from(msg, "utf8").toString("base64");
//   const base64Msg = new TextEncoder().encode(msg)
  const utf8Bytes = new TextEncoder().encode(msg);

  let binary = "";
  for (let i = 0; i < utf8Bytes.length; i++) {
    binary += String.fromCharCode(utf8Bytes[i]);
  }
  const base64Msg = btoa(binary);

  log("✉️ sending:", msg, "→", base64Msg);

  await device.writeCharacteristicWithResponseForService(
    SERVICE_UUID,
    WRITE_CHAR_UUID,
    base64Msg
  );

  log("✅ write done");
}
