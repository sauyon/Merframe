import { type ReactNode, useCallback, useState } from "react";
import { PolarityIcon } from "@/components/game-icon";
import { Button } from "@/components/ui/button";
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from "@/components/ui/dialog";
import { Field } from "@/components/ui/field";
import { Input } from "@/components/ui/input";
import { Slider } from "@/components/ui/slider";
import { SwitchField } from "@/components/ui/switch";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Textarea } from "@/components/ui/textarea";
import { api, reportError } from "@/lib/bridge";
import { counted } from "@/lib/format";
import { notify } from "@/lib/toast";
import type { ListingChoices, RivenRow } from "@/types";
import { MAX_RIVEN_RANK } from "./columns";

export interface ListingForm {
  direct: boolean;
  visible: boolean;
  rank: number;
  sellingPrice: string;
  startingPrice: string;
  buyoutPrice: string;
  minReputation: string;
  note: string;
}

const EMPTY_LISTING: ListingForm = {
  direct: true,
  visible: true,
  rank: 0,
  sellingPrice: "0",
  startingPrice: "0",
  buyoutPrice: "0",
  minReputation: "0",
  note: "",
};

function wholeNumber(value: string): number {
  const parsed = Number.parseInt(value, 10);
  return Number.isFinite(parsed) && parsed > 0 ? parsed : 0;
}

function listingChoices(form: ListingForm): ListingChoices {
  return {
    direct: form.direct,
    visible: form.visible,
    rank: form.rank,
    sellingPrice: wholeNumber(form.sellingPrice),
    startingPrice: wholeNumber(form.startingPrice),
    buyoutPrice: wholeNumber(form.buyoutPrice),
    minReputation: wholeNumber(form.minReputation),
    note: form.note.trim(),
  };
}

function priced(form: ListingForm): boolean {
  return wholeNumber(form.direct ? form.sellingPrice : form.startingPrice) > 0;
}

function buyoutAboveStarting(form: ListingForm): ListingForm {
  const starting = wholeNumber(form.startingPrice);
  const buyout = wholeNumber(form.buyoutPrice);
  return buyout > 0 && buyout <= starting
    ? { ...form, buyoutPrice: String(starting + 1) }
    : form;
}

function PriceField({
  id,
  label,
  value,
  onChange,
  onBlur,
}: {
  id: string;
  label: string;
  value: string;
  onChange: (value: string) => void;
  onBlur?: () => void;
}) {
  return (
    <Field id={id} label={label}>
      <Input
        id={id}
        value={value}
        onChange={(e) => onChange(e.target.value)}
        onBlur={onBlur}
        inputMode="numeric"
      />
    </Field>
  );
}

export function ListingDialog({
  title,
  description,
  initial,
  rankEditable,
  submitLabel,
  onSubmit,
  onClose,
}: {
  title: string;
  description: ReactNode;
  initial: ListingForm;
  rankEditable?: boolean;
  submitLabel?: string;
  onSubmit: (choices: ListingChoices) => Promise<void>;
  onClose: () => void;
}) {
  const [form, setForm] = useState(initial);
  const [posting, setPosting] = useState(false);

  const handleSubmit = useCallback(async () => {
    setPosting(true);
    await onSubmit(listingChoices(form));
    setPosting(false);
    onClose();
  }, [form, onSubmit, onClose]);

  return (
    <Dialog open onOpenChange={(open) => !open && onClose()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>{title}</DialogTitle>
          <DialogDescription className="flex items-center gap-2">
            {description}
          </DialogDescription>
        </DialogHeader>
        <div className="flex flex-col gap-4">
          <div className="flex items-center justify-between gap-4">
            <Tabs
              value={form.direct ? "direct" : "auction"}
              onValueChange={(value) =>
                setForm({ ...form, direct: value === "direct" })
              }
            >
              <TabsList>
                <TabsTrigger value="direct">Direct sale</TabsTrigger>
                <TabsTrigger value="auction">Auction</TabsTrigger>
              </TabsList>
            </Tabs>
            <SwitchField
              id="visible"
              label="Visible"
              checked={form.visible}
              onChange={(visible) => setForm({ ...form, visible })}
            />
          </div>

          {form.direct ? (
            <PriceField
              id="selling"
              label="Selling price"
              value={form.sellingPrice}
              onChange={(sellingPrice) => setForm({ ...form, sellingPrice })}
            />
          ) : (
            <div className="grid grid-cols-3 gap-4">
              <PriceField
                id="starting"
                label="Starting price"
                value={form.startingPrice}
                onChange={(startingPrice) =>
                  setForm({ ...form, startingPrice })
                }
                onBlur={() => setForm(buyoutAboveStarting)}
              />
              <PriceField
                id="buyout"
                label="Buyout"
                value={form.buyoutPrice}
                onChange={(buyoutPrice) => setForm({ ...form, buyoutPrice })}
                onBlur={() => setForm(buyoutAboveStarting)}
              />
              <PriceField
                id="reputation"
                label="Minimum reputation"
                value={form.minReputation}
                onChange={(minReputation) =>
                  setForm({ ...form, minReputation })
                }
              />
            </div>
          )}

          {rankEditable && (
            <Field id="rank" label={`Mod rank ${form.rank}`}>
              <Slider
                id="rank"
                min={0}
                max={MAX_RIVEN_RANK}
                step={1}
                value={[form.rank]}
                onValueChange={([rank = form.rank]) =>
                  setForm({ ...form, rank })
                }
              />
            </Field>
          )}

          <Field id="note" label="Note">
            <Textarea
              id="note"
              value={form.note}
              onChange={(e) => setForm({ ...form, note: e.target.value })}
              placeholder="Optional note shown on the auction"
            />
          </Field>
        </div>
        <DialogFooter>
          <Button variant="outline" onClick={onClose}>
            Cancel
          </Button>
          <Button
            onClick={handleSubmit}
            disabled={!priced(form)}
            pending={posting}
          >
            {submitLabel ?? (form.direct ? "List for Sale" : "Post Auction")}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

export function RivenListingDialog({
  riven,
  onClose,
}: {
  riven: RivenRow;
  onClose: () => void;
}) {
  const handleSubmit = useCallback(
    async (choices: ListingChoices) => {
      try {
        await api.marketPostRiven(riven.item_id, choices);
        notify.success("Listed on warframe.market");
      } catch (error) {
        reportError(error);
      }
    },
    [riven.item_id],
  );

  return (
    <ListingDialog
      title="List on warframe.market"
      description={
        <>
          <span>
            {`${riven.weapon} ${riven.name ?? ""}, ${counted(riven.rerolls, "roll")}`}
          </span>
          {riven.polarity && (
            <PolarityIcon polarity={riven.polarity} size={20} />
          )}
          {riven.unlisted_stat && (
            <span className="text-warning">
              {`warframe.market has no attribute for ${riven.unlisted_stat} yet, so it stays off the listing`}
            </span>
          )}
        </>
      }
      initial={{ ...EMPTY_LISTING, rank: riven.rank }}
      rankEditable
      onSubmit={handleSubmit}
      onClose={onClose}
    />
  );
}
