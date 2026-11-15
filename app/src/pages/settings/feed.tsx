import { Section } from "@/components/page";
import { Field } from "@/components/ui/field";
import { Hint } from "@/components/ui/hint";
import { NumberInput } from "@/components/ui/number-input";
import { useAppStore } from "@/stores/app-store";
import type { EventFeedStatus, Settings } from "@/types";
import { type Patch, SwitchRow } from "./row";

function feedHint(status: EventFeedStatus | undefined) {
  switch (status?.state) {
    case "listening":
      return `Streaming at http://127.0.0.1:${status.port}/events`;
    case "failed":
      return `Listening on port ${status.port} failed: ${status.cause}`;
    case "off":
    case undefined:
      return "Not running";
  }
}

export function EventFeed({ draft, patch }: { draft: Settings; patch: Patch }) {
  const status = useAppStore((state) => state.status);

  return (
    <Section
      title="Event Feed"
      description="Other programs on this computer can follow what Merframe sees as a stream of server-sent events."
    >
      <div className="flex flex-col gap-4">
        <SwitchRow
          checked={draft.event_feed_enabled}
          onChange={(checked) => patch({ event_feed_enabled: checked })}
          hint="Allows third-parties to react to game status, inventory changes, relic rewards, trades and alerts. Personal account information is excluded."
        >
          Stream events for other programs on this PC
        </SwitchRow>
        <Field id="event-feed-port" label="Port" inline>
          <NumberInput
            id="event-feed-port"
            disabled={!draft.event_feed_enabled}
            value={draft.event_feed_port}
            onChange={(port) => patch({ event_feed_port: port })}
            className="w-24"
          />
        </Field>
        <Hint>{feedHint(status?.event_feed)}</Hint>
      </div>
    </Section>
  );
}
