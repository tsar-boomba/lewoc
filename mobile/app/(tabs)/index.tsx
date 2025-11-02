// app/(tabs)/index.tsx
import React, { useState } from "react";
import {
  SafeAreaView,
  View,
  Text,
  TextInput,
  TouchableOpacity,
  StyleSheet,
  KeyboardAvoidingView,
  Platform,
  Alert,
  Modal,
  FlatList,
} from "react-native";
import { Device } from "react-native-ble-plx";
import {
  scanForDevices,
  connectToDevice,
  sendMessageToDevice,
} from "../../lib/bluetooth";

const MAX_LEN = 128;

export default function HomeScreen() {
  const [connected, setConnected] = useState(false);
  const [device, setDevice] = useState<Device | null>(null);
  const [message, setMessage] = useState("");
  const [scanning, setScanning] = useState(false);
  const [sending, setSending] = useState(false);
  const [devices, setDevices] = useState<Device[]>([]);
  const [modalVisible, setModalVisible] = useState(false);

  const handleScan = async () => {
    setScanning(true);
    try {
      const found = await scanForDevices(); // now 4s
      setDevices(found);
      setModalVisible(true);
    } catch (e: any) {
      Alert.alert("Bluetooth", e?.message ?? "Scan failed");
    } finally {
      setScanning(false);
    }
  };

  const handleSelectDevice = async (d: Device) => {
    setModalVisible(false);
    try {
      const conn = await connectToDevice(d);
      setDevice(conn);
      setConnected(true);
    } catch (e: any) {
      Alert.alert("Bluetooth", e?.message ?? "Failed to connect");
      setConnected(false);
      setDevice(null);
    }
  };

  const handleSend = async () => {
    if (!device) return;
    if (!message.trim()) return;
    if (message.length > MAX_LEN) {
      Alert.alert("Message too long", `Limit is ${MAX_LEN} characters.`);
      return;
    }

    setSending(true);

    // send with timeout (3s)
    const sendPromise = sendMessageToDevice(device, message.trim());
    const timeoutPromise = new Promise((_, reject) =>
      setTimeout(() => reject(new Error("Send timed out")), 3000)
    );

    try {
      await Promise.race([sendPromise, timeoutPromise]);
      setMessage("");
    } catch (e: any) {
      Alert.alert(
        "Send failed",
        e?.message ?? "Message was not received in time."
      );
    } finally {
      setSending(false);
    }
  };

  const onChangeMessage = (text: string) => {
    // hard cap at 128
    if (text.length <= MAX_LEN) {
      setMessage(text);
    } else {
      // or trim silently
      setMessage(text.slice(0, MAX_LEN));
    }
  };

  return (
    <SafeAreaView style={styles.safe}>
      <KeyboardAvoidingView
        style={styles.container}
        behavior={Platform.OS === "ios" ? "padding" : undefined}
      >
        {/* Header */}
        <View style={styles.header}>
          <Text style={styles.title}>lewoc</Text>

          <View style={styles.statusContainer}>
            <View
              style={[
                styles.statusDot,
                { backgroundColor: connected ? "#22c55e" : "#9ca3af" },
              ]}
            />
            <Text style={styles.statusText}>
              {connected ? "Connected" : "Not connected"}
            </Text>
          </View>
        </View>

        {/* Scan / connect */}
        <View style={styles.connectContainer}>
          <TouchableOpacity
            style={[styles.button, scanning && styles.buttonDisabled]}
            onPress={handleScan}
            disabled={scanning}
          >
            <Text style={styles.buttonText}>
              {scanning ? "Scanning..." : "Scan for devices"}
            </Text>
          </TouchableOpacity>
          <Text style={styles.helperText}>
            Pick from nearby devices (BLE, same service UUID)
          </Text>
        </View>

        {/* Bottom input */}
        <View style={styles.bottomContainer}>
          <View style={{ flex: 1 }}>
            <TextInput
              style={styles.input}
              placeholder={
                connected ? "Type your message..." : "Connect first..."
              }
              value={message}
              onChangeText={onChangeMessage}
              editable={connected && !sending}
              multiline
            />
            <Text
              style={[
                styles.charCount,
                message.length > MAX_LEN && { color: "red" },
              ]}
            >
              {message.length}/{MAX_LEN}
            </Text>
          </View>
          <TouchableOpacity
            style={[
              styles.sendButton,
              (!connected ||
                !message.trim() ||
                sending ||
                message.length > MAX_LEN) &&
                styles.buttonDisabled,
            ]}
            onPress={handleSend}
            disabled={
              !connected ||
              !message.trim() ||
              sending ||
              message.length > MAX_LEN
            }
          >
            <Text style={styles.sendText}>{sending ? "..." : "Send"}</Text>
          </TouchableOpacity>
        </View>

        {/* Device picker modal */}
        <Modal visible={modalVisible} transparent animationType="slide">
          <View style={styles.modalContainer}>
            <View style={styles.modal}>
              <Text style={styles.modalTitle}>Select a device</Text>
              <FlatList
                data={devices}
                keyExtractor={(item) => item.id}
                renderItem={({ item }) => (
                  <TouchableOpacity
                    style={styles.deviceItem}
                    onPress={() => handleSelectDevice(item)}
                  >
                    <Text style={styles.deviceText}>
                      {item.name ?? "Unnamed"} ({item.id})
                    </Text>
                  </TouchableOpacity>
                )}
                ListEmptyComponent={
                  <Text style={{ color: "#6b7280" }}>
                    No devices found. Make sure it’s advertising.
                  </Text>
                }
              />
              <TouchableOpacity
                style={[styles.button, { marginTop: 10 }]}
                onPress={() => setModalVisible(false)}
              >
                <Text style={styles.buttonText}>Close</Text>
              </TouchableOpacity>
            </View>
          </View>
        </Modal>
      </KeyboardAvoidingView>
    </SafeAreaView>
  );
}

const styles = StyleSheet.create({
  safe: { flex: 1, backgroundColor: "#fff" },
  container: { flex: 1, justifyContent: "space-between" },
  header: { alignItems: "center", marginTop: 20 },
  title: { fontSize: 28, fontWeight: "700", color: "#111827" },
  statusContainer: { flexDirection: "row", alignItems: "center", marginTop: 10 },
  statusDot: { width: 10, height: 10, borderRadius: 5, marginRight: 8 },
  statusText: { fontSize: 15, color: "#6b7280" },
  connectContainer: {
    flex: 1,
    alignItems: "center",
    justifyContent: "center",
    gap: 8,
  },
  button: {
    backgroundColor: "#2563eb",
    paddingVertical: 14,
    paddingHorizontal: 32,
    borderRadius: 12,
  },
  buttonDisabled: {
    backgroundColor: "#94a3b8",
  },
  buttonText: { color: "#fff", fontWeight: "600" },
  helperText: { color: "#6b7280", fontSize: 13 },
  bottomContainer: {
    flexDirection: "row",
    alignItems: "center",
    gap: 10,
    paddingHorizontal: 16,
    paddingBottom: Platform.OS === "ios" ? 24 : 16,
  },
  input: {
    borderWidth: 1,
    borderColor: "#e5e7eb",
    borderRadius: 20,
    paddingHorizontal: 16,
    paddingVertical: 10,
    fontSize: 16,
    backgroundColor: "#f9fafb",
    minHeight: 48,
  },
  charCount: {
    alignSelf: "flex-end",
    fontSize: 12,
    color: "#6b7280",
    marginTop: 4,
    marginRight: 4,
  },
  sendButton: {
    backgroundColor: "#2563eb",
    paddingVertical: 12,
    paddingHorizontal: 20,
    borderRadius: 20,
    marginBottom:16,
  },
  sendText: { color: "#fff", fontWeight: "600", fontSize: 16 },
  modalContainer: {
    flex: 1,
    backgroundColor: "rgba(0,0,0,0.4)",
    justifyContent: "center",
    alignItems: "center",
  },
  modal: {
    backgroundColor: "#fff",
    width: "80%",
    borderRadius: 10,
    padding: 18,
    maxHeight: "70%",
  },
  modalTitle: { fontSize: 18, fontWeight: "600", marginBottom: 8 },
  deviceItem: {
    paddingVertical: 10,
    borderBottomWidth: 1,
    borderBottomColor: "#e5e7eb",
  },
  deviceText: { fontSize: 15 },
});
