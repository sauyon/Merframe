import { LogOut, Trash2 } from "lucide-react";
import { type ReactNode, useState } from "react";
import { Section } from "@/components/page";
import { Button } from "@/components/ui/button";
import { ConfirmDialog } from "@/components/ui/confirm-dialog";
import { Field } from "@/components/ui/field";
import { Hint } from "@/components/ui/hint";
import { Input } from "@/components/ui/input";
import { api, reportError } from "@/lib/bridge";
import { signOutOfMarket } from "@/lib/market-session";
import { notify } from "@/lib/toast";

const RESET_WORD = "RESET";

function DangerRow({
  title,
  hint,
  children,
}: {
  title: string;
  hint: string;
  children: ReactNode;
}) {
  return (
    <div className="flex flex-col gap-2">
      <span className="text-sm font-medium">{title}</span>
      <div className="flex flex-wrap items-center gap-3">{children}</div>
      <Hint>{hint}</Hint>
    </div>
  );
}

function ForgetMarketLogin() {
  const [confirming, setConfirming] = useState(false);
  const [busy, setBusy] = useState(false);

  const forget = async () => {
    setBusy(true);
    try {
      await signOutOfMarket();
      setConfirming(false);
      notify.success("Saved warframe.market login removed");
    } catch (error) {
      reportError(error);
    } finally {
      setBusy(false);
    }
  };

  return (
    <DangerRow
      title="Forget warframe.market login"
      hint="Signs you out and removes your saved login from this computer. Your other Merframe data stays."
    >
      <Button variant="outline" onClick={() => setConfirming(true)}>
        <LogOut className="size-4" />
        Forget Login
      </Button>
      <ConfirmDialog
        open={confirming}
        onOpenChange={setConfirming}
        title="Forget Login"
        description="Signs you out of warframe.market and removes the saved login from this computer. You'll need to sign in again to manage listings."
        confirmLabel="Forget Login"
        busy={busy}
        onConfirm={forget}
      />
    </DangerRow>
  );
}

function ResetMerframe() {
  const [confirming, setConfirming] = useState(false);
  const [typed, setTyped] = useState("");
  const [busy, setBusy] = useState(false);

  const reset = async () => {
    setBusy(true);
    try {
      await api.resetMerframe();
    } catch (error) {
      reportError(error);
      setBusy(false);
    }
  };

  return (
    <DangerRow
      title="Reset Merframe"
      hint="Deletes all Merframe data, including your login, settings, inventory snapshots, trade and relic history, statistics, favourites, logs and caches."
    >
      <Button variant="destructive" onClick={() => setConfirming(true)}>
        <Trash2 className="size-4" />
        Reset Merframe
      </Button>
      <ConfirmDialog
        open={confirming}
        onOpenChange={(open) => {
          setConfirming(open);
          setTyped("");
        }}
        title="Reset Merframe"
        description="All Merframe data on this computer will be deleted! This includes your market login, settings, history, statistics, favourites, logs, caches and interface preferences. Merframe restarts afterwards. This cannot be undone."
        confirmLabel="Reset Merframe"
        busy={busy}
        destructive
        confirmDisabled={typed !== RESET_WORD}
        onConfirm={reset}
      >
        <Field id="reset-confirmation" label={`Type ${RESET_WORD} to confirm`}>
          <Input
            id="reset-confirmation"
            value={typed}
            disabled={busy}
            autoComplete="off"
            spellCheck={false}
            onChange={(event) => setTyped(event.target.value)}
          />
        </Field>
      </ConfirmDialog>
    </DangerRow>
  );
}

export function DangerZone() {
  return (
    <Section
      id="danger-zone"
      title="Danger Zone"
      description="Remove your saved warframe.market login or delete your Merframe data."
    >
      <div className="grid gap-x-8 gap-y-6 @3xl:grid-cols-2">
        <ForgetMarketLogin />
        <ResetMerframe />
      </div>
    </Section>
  );
}
