import { useState } from "react";
import { api, errorMessage } from "@/data/api";
import type { Account } from "@/data/types";

interface SignInFormProps {
  /** One line saying why the writer is being asked. */
  prompt: string;
  onSignedIn: (account: Account) => void;
}

/** Email and password, the one sign-in method the website offers. */
export function SignInForm({ prompt, onSignedIn }: SignInFormProps) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState("");

  const submit = async (event: React.FormEvent) => {
    event.preventDefault();
    setBusy(true);
    setError("");
    try {
      onSignedIn(await api.signIn(email, password));
    } catch (e) {
      setError(errorMessage(e));
    } finally {
      setBusy(false);
    }
  };

  return (
    <form onSubmit={submit} className="space-y-3">
      <p className="text-ns-ink-secondary">{prompt}</p>
      <input
        className="field"
        type="email"
        required
        autoFocus
        autoComplete="username"
        placeholder="Email"
        aria-label="Email"
        value={email}
        onChange={(event) => setEmail(event.target.value)}
      />
      <input
        className="field"
        type="password"
        required
        autoComplete="current-password"
        placeholder="Password"
        aria-label="Password"
        value={password}
        onChange={(event) => setPassword(event.target.value)}
      />
      {error && (
        <p role="alert" className="text-ns-destructive">
          {error}
        </p>
      )}
      <button type="submit" className="btn btn-primary" disabled={busy}>
        {busy ? "Signing in…" : "Sign in"}
      </button>
    </form>
  );
}
