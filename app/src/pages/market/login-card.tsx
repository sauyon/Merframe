import { useCallback, useState } from "react";
import { toast } from "sonner";
import { ErrorNote, Section } from "@/components/page";
import { Button } from "@/components/ui/button";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { api, errorMessage } from "@/lib/bridge";
import { notify } from "@/lib/toast";

export function LoginCard({ onDone }: { onDone: () => void }) {
  const [email, setEmail] = useState("");
  const [password, setPassword] = useState("");
  const [busy, setBusy] = useState(false);
  const [failure, setFailure] = useState<string | null>(null);

  const handleSubmit = useCallback(async () => {
    setBusy(true);
    setFailure(null);
    try {
      const account = await api.marketLogin(email, password);
      notify.success(`Signed in as ${account.ingame_name}`);
      onDone();
    } catch (error) {
      const message = errorMessage(error);
      setFailure(message);
      toast.error(message);
    } finally {
      setBusy(false);
    }
  }, [email, password, onDone]);

  return (
    <Section
      title="Sign In"
      description="Your credentials go straight to warframe.market. Merframe keeps only the session token."
      className="max-w-xl"
    >
      <div className="flex flex-col gap-4">
        <Field id="email" label="Email">
          <Input
            id="email"
            type="email"
            value={email}
            onChange={(e) => setEmail(e.target.value)}
            autoComplete="username"
          />
        </Field>
        <Field id="password" label="Password">
          <Input
            id="password"
            type="password"
            value={password}
            onChange={(e) => setPassword(e.target.value)}
            autoComplete="current-password"
          />
        </Field>
        {failure && <ErrorNote message={failure} />}
        <Button
          onClick={handleSubmit}
          disabled={!email || !password}
          pending={busy}
        >
          Sign In
        </Button>
      </div>
    </Section>
  );
}
