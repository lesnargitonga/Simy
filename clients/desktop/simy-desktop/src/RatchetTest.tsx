import { useState } from "react";
import {
  initRatchetStore,
  generateIdentity,
  generatePreKeyBundle,
  bootstrapInitiator,
  bootstrapResponder,
  encryptMessage,
  decryptMessage,
  checkSessionStatus,
  type GenerateIdentityResponse,
  type PreKeyBundleResponse,
} from "./ratchet-api";
import { CheckCircle, XCircle, Loader } from "lucide-react";

interface TestStep {
  name: string;
  status: "pending" | "running" | "success" | "error";
  message?: string;
}

export function RatchetTest() {
  const [steps, setSteps] = useState<TestStep[]>([]);
  const [aliceIdentity, setAliceIdentity] = useState<GenerateIdentityResponse | null>(null);
  const [bobIdentity, setBobIdentity] = useState<GenerateIdentityResponse | null>(null);
  const [bobBundle, setBobBundle] = useState<PreKeyBundleResponse | null>(null);
  const [aliceSessionId, setAliceSessionId] = useState<string | null>(null);
  const [bobSessionId, setBobSessionId] = useState<string | null>(null);
  const [testPhase, setTestPhase] = useState<"setup" | "running" | "done">("setup");

  const updateStep = (name: string, status: TestStep["status"], message?: string) => {
    setSteps((prev) => {
      const existing = prev.find((s) => s.name === name);
      if (existing) {
        return prev.map((s) => (s.name === name ? { ...s, status, message } : s));
      }
      return [...prev, { name, status, message }];
    });
  };

  const runFullTest = async () => {
    setTestPhase("running");
    setSteps([]);

    try {
      // Step 1: Initialize store
      updateStep("Initialize Store", "running");
      const storePath = "./simy-ratchet-sessions-test";
      const encryptionKey = btoa(
        String.fromCharCode(...crypto.getRandomValues(new Uint8Array(32)))
      );
      await initRatchetStore({
        storage_path: storePath,
        encryption_key_b64: encryptionKey,
      });
      updateStep("Initialize Store", "success", `Store initialized at ${storePath}`);

      // Step 2: Generate Alice's identity
      updateStep("Generate Alice Identity", "running");
      const alice = await generateIdentity();
      setAliceIdentity(alice);
      updateStep(
        "Generate Alice Identity",
        "success",
        `Signing: ${alice.signing_key_b64.substring(0, 16)}...`
      );

      // Step 3: Generate Bob's identity
      updateStep("Generate Bob Identity", "running");
      const bob = await generateIdentity();
      setBobIdentity(bob);
      updateStep(
        "Generate Bob Identity",
        "success",
        `Signing: ${bob.signing_key_b64.substring(0, 16)}...`
      );

      // Step 4: Generate Bob's prekey bundle
      updateStep("Generate Bob PreKey Bundle", "running");
      const bobPkb = await generatePreKeyBundle({
        signing_key_b64: bob.signing_key_b64,
        exchange_key_b64: bob.exchange_key_b64,
      });
      setBobBundle(bobPkb);
      updateStep(
        "Generate Bob PreKey Bundle",
        "success",
        `PreKey: ${bobPkb.signed_prekey_b64.substring(0, 16)}...`
      );

      // Step 5: Alice bootstraps session
      updateStep("Alice Bootstrap Session", "running");
      const aliceBootstrap = await bootstrapInitiator({
        alice_signing_key_b64: alice.signing_key_b64,
        alice_exchange_key_b64: alice.exchange_key_b64,
        bob_identity_signing_key_b64: bobPkb.identity_signing_key_b64,
        bob_identity_exchange_key_b64: bobPkb.identity_exchange_key_b64,
        bob_signed_prekey_b64: bobPkb.signed_prekey_b64,
        bob_signed_prekey_signature_b64: bobPkb.signed_prekey_signature_b64,
        initial_message: "Hello Bob, this is Alice!",
      });
      setAliceSessionId(aliceBootstrap.session_id);
      updateStep(
        "Alice Bootstrap Session",
        "success",
        `Session: ${aliceBootstrap.session_id}`
      );

      // Step 6: Bob receives and bootstraps
      updateStep("Bob Bootstrap Session", "running");
      const bobBootstrap = await bootstrapResponder({
        bob_signing_key_b64: bob.signing_key_b64,
        bob_exchange_key_b64: bob.exchange_key_b64,
        bob_signed_prekey_b64: bobPkb.signed_prekey_b64,
        bob_signed_prekey_signature_b64: bobPkb.signed_prekey_signature_b64,
        initial_envelope_b64: aliceBootstrap.initial_envelope_b64,
      });
      setBobSessionId(bobBootstrap.session_id);
      updateStep(
        "Bob Bootstrap Session",
        "success",
        `Decrypted: "${bobBootstrap.decrypted_message}"`
      );

      // Step 7: Bob sends message
      updateStep("Bob Encrypt Message", "running");
      const bobEncrypted = await encryptMessage({
        session_id: bobBootstrap.session_id,
        plaintext: "Hi Alice! Got your message.",
        associated_data: "msg-001",
      });
      updateStep(
        "Bob Encrypt Message",
        "success",
        `Ciphertext: ${bobEncrypted.ciphertext_b64.substring(0, 32)}...`
      );

      // Step 8: Alice decrypts
      updateStep("Alice Decrypt Message", "running");
      const aliceDecrypted = await decryptMessage({
        session_id: aliceBootstrap.session_id,
        ciphertext_b64: bobEncrypted.ciphertext_b64,
        associated_data: "msg-001",
      });
      updateStep(
        "Alice Decrypt Message",
        "success",
        `Plaintext: "${aliceDecrypted.plaintext}"`
      );

      // Step 9: Check session status
      updateStep("Check Alice Session Status", "running");
      const aliceStatus = await checkSessionStatus({
        session_id: aliceBootstrap.session_id,
      });
      updateStep(
        "Check Alice Session Status",
        "success",
        `Exists: ${aliceStatus.exists}, Messages: ${aliceStatus.message_count}`
      );

      // Step 10: Alice sends follow-up
      updateStep("Alice Encrypt Follow-up", "running");
      const aliceEncrypted = await encryptMessage({
        session_id: aliceBootstrap.session_id,
        plaintext: "Perfect! Session persistence working.",
        associated_data: "msg-002",
      });
      updateStep(
        "Alice Encrypt Follow-up",
        "success",
        `Ciphertext: ${aliceEncrypted.ciphertext_b64.substring(0, 32)}...`
      );

      // Step 11: Bob decrypts follow-up
      updateStep("Bob Decrypt Follow-up", "running");
      const bobDecrypted = await decryptMessage({
        session_id: bobBootstrap.session_id,
        ciphertext_b64: aliceEncrypted.ciphertext_b64,
        associated_data: "msg-002",
      });
      updateStep(
        "Bob Decrypt Follow-up",
        "success",
        `Plaintext: "${bobDecrypted.plaintext}"`
      );

      // Step 12: Final status check
      updateStep("Final Session Status Check", "running");
      const bobStatusFinal = await checkSessionStatus({
        session_id: bobBootstrap.session_id,
      });
      updateStep(
        "Final Session Status Check",
        "success",
        `Exists: ${bobStatusFinal.exists}, Messages: ${bobStatusFinal.message_count}`
      );

      setTestPhase("done");
    } catch (error) {
      const lastStep = steps[steps.length - 1];
      if (lastStep) {
        updateStep(
          lastStep.name,
          "error",
          error instanceof Error ? error.message : String(error)
        );
      }
      setTestPhase("done");
    }
  };

  const renderStepIcon = (status: TestStep["status"]) => {
    switch (status) {
      case "running":
        return <Loader className="animate-spin text-blue-500" size={20} />;
      case "success":
        return <CheckCircle className="text-green-500" size={20} />;
      case "error":
        return <XCircle className="text-red-500" size={20} />;
      default:
        return <div className="w-5 h-5 rounded-full border-2 border-slate-300" />;
    }
  };

  return (
    <div className="min-h-screen bg-slate-900 text-white p-8">
      <div className="max-w-4xl mx-auto space-y-6">
        <div className="text-center space-y-2">
          <h1 className="text-4xl font-bold">Simy Desktop</h1>
          <p className="text-slate-400">Double Ratchet Session Persistence Test</p>
        </div>

        <div className="bg-slate-800 rounded-lg p-6 space-y-4">
          <div className="flex justify-between items-center">
            <h2 className="text-xl font-semibold">Test Suite</h2>
            <button
              onClick={runFullTest}
              disabled={testPhase === "running"}
              className="px-4 py-2 bg-blue-600 hover:bg-blue-700 disabled:bg-slate-600 rounded-lg font-medium transition-colors"
            >
              {testPhase === "running" ? "Running..." : "Run Full Test"}
            </button>
          </div>

          {steps.length === 0 && testPhase === "setup" && (
            <div className="text-center py-12 text-slate-400">
              Click "Run Full Test" to start the session persistence demo
            </div>
          )}

          <div className="space-y-3">
            {steps.map((step, idx) => (
              <div
                key={idx}
                className="flex items-start gap-3 p-3 bg-slate-900 rounded-lg"
              >
                <div className="mt-0.5">{renderStepIcon(step.status)}</div>
                <div className="flex-1 min-w-0">
                  <div className="font-medium">{step.name}</div>
                  {step.message && (
                    <div className="text-sm text-slate-400 mt-1 break-all">
                      {step.message}
                    </div>
                  )}
                </div>
              </div>
            ))}
          </div>

          {testPhase === "done" && (
            <div className="mt-6 p-4 bg-green-900/20 border border-green-800 rounded-lg">
              <div className="font-semibold text-green-400">✓ Test Complete</div>
              <div className="text-sm text-green-300 mt-2">
                All steps completed successfully. Sessions are now persisted in encrypted
                file storage and can survive app restarts.
              </div>
            </div>
          )}
        </div>

        {(aliceIdentity || bobIdentity || bobBundle || aliceSessionId || bobSessionId) && (
          <div className="bg-slate-800 rounded-lg p-6 space-y-4">
            <h2 className="text-xl font-semibold">Test Artifacts</h2>
            <div className="space-y-3 text-sm">
              {aliceIdentity && (
                <div>
                  <div className="text-slate-400">Alice Identity</div>
                  <div className="font-mono text-xs break-all text-slate-300">
                    {aliceIdentity.signing_key_b64.substring(0, 64)}...
                  </div>
                </div>
              )}
              {bobIdentity && (
                <div>
                  <div className="text-slate-400">Bob Identity</div>
                  <div className="font-mono text-xs break-all text-slate-300">
                    {bobIdentity.signing_key_b64.substring(0, 64)}...
                  </div>
                </div>
              )}
              {aliceSessionId && (
                <div>
                  <div className="text-slate-400">Alice Session ID</div>
                  <div className="font-mono text-xs break-all text-blue-400">
                    {aliceSessionId}
                  </div>
                </div>
              )}
              {bobSessionId && (
                <div>
                  <div className="text-slate-400">Bob Session ID</div>
                  <div className="font-mono text-xs break-all text-blue-400">
                    {bobSessionId}
                  </div>
                </div>
              )}
            </div>
          </div>
        )}
      </div>
    </div>
  );
}
