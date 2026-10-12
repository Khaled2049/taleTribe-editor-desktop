import { CircleUser } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { api, errorMessage } from "@/data/api";
import type { Account } from "@/data/types";
import { SignInForm } from "./SignInForm";

/**
 * Shows who is signed in to the website and lets the writer sign in or out.
 * Signing in is never required to write; it is only used to publish.
 */
export function AccountButton() {
  // Undefined while loading, null when signed out.
  const [account, setAccount] = useState<Account | null | undefined>(undefined);
  const [open, setOpen] = useState(false);
  const [error, setError] = useState("");
  const dialog = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    api
      .getAccount()
      .then(setAccount)
      .catch(() => setAccount(null));
  }, []);

  useEffect(() => {
    if (!open) return;
    const element = dialog.current;
    element?.showModal();
    return () => element?.close();
  }, [open]);

  const close = () => {
    setOpen(false);
    setError("");
  };

  const signOut = async () => {
    try {
      await api.signOut();
      setAccount(null);
      close();
    } catch (e) {
      setError(errorMessage(e));
    }
  };

  if (account === undefined) return null;
  return (
    <>
      <button
        type="button"
        className="btn max-w-[16rem]"
        onClick={() => setOpen(true)}
      >
        <CircleUser size={16} className="shrink-0" />
        <span className="truncate">{account ? account.email : "Sign in"}</span>
      </button>

      {open && (
        <dialog
          ref={dialog}
          onCancel={(event) => {
            event.preventDefault();
            close();
          }}
          className="w-[26rem] rounded-ns-lg border border-ns-border bg-ns-elevated p-5 text-ns-ink backdrop:bg-black/40"
        >
          <h2 className="mb-3 text-base font-semibold">TheTaleTribe account</h2>
          {account ? (
            <div className="space-y-3">
              <p>
                Signed in as <span className="font-medium">{account.email}</span>.
              </p>
              <p className="text-ns-ink-secondary">
                Signing out keeps every story on this computer. You will be
                asked to sign in again the next time you publish.
              </p>
              {error && (
                <p role="alert" className="text-ns-destructive">
                  {error}
                </p>
              )}
            </div>
          ) : (
            <SignInForm
              prompt="Sign in to publish your stories to the website. You don't need an account to write here."
              onSignedIn={(signedIn) => {
                setAccount(signedIn);
                close();
              }}
            />
          )}
          <div className="mt-5 flex justify-end gap-2">
            <button type="button" className="btn" onClick={close}>
              Close
            </button>
            {account && (
              <button type="button" className="btn" onClick={() => void signOut()}>
                Sign out
              </button>
            )}
          </div>
        </dialog>
      )}
    </>
  );
}
