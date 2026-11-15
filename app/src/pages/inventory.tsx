import { Download } from "lucide-react";
import { useCallback, useMemo, useState } from "react";
import { InventoryTabView } from "@/components/inventory-tab-view";
import { Page, Section } from "@/components/page";
import { Button } from "@/components/ui/button";
import { Tabs, TabsContent, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { useAsyncData } from "@/hooks/use-async-data";
import { useTabParam } from "@/hooks/use-tab-param";
import { api, reportError } from "@/lib/bridge";
import { num } from "@/lib/format";
import { entriesFor, inventoryFlags } from "@/lib/inventory-entries";
import type { InventoryTabKey } from "@/lib/inventory-filters";
import { notify } from "@/lib/toast";

export function InventoryPage() {
  const { data, error, loading } = useAsyncData(
    "inventory_tab",
    api.inventoryTab,
    ["inventoryUpdated", "pricesUpdated", "favouriteUpdated", "marketUpdated"],
  );
  const [exporting, setExporting] = useState(false);
  const [tab, setTab] = useTabParam<InventoryTabKey>("parts");

  const entries = useMemo(() => entriesFor(tab, data), [tab, data]);
  const flags = useMemo(() => inventoryFlags(data), [data]);

  const handleExport = useCallback(async () => {
    setExporting(true);
    try {
      const dir = await api.exportBundle();
      if (dir) {
        notify.success("Exported", { description: dir });
      }
    } catch (error) {
      reportError(error);
    } finally {
      setExporting(false);
    }
  }, []);

  return (
    <Page
      title="Inventory"
      quote="inventory"
      actions={
        <Button variant="outline" onClick={handleExport} pending={exporting}>
          <Download className="size-4" />
          Export JSON
        </Button>
      }
      loading={loading && !data}
      error={error}
    >
      <Section>
        <Tabs value={tab} onValueChange={setTab}>
          <TabsList className="mb-4">
            <TabsTrigger value="parts">
              All Parts ({num(data?.parts.length ?? 0)})
            </TabsTrigger>
            <TabsTrigger value="relics">
              Relics ({num(data?.relics.length ?? 0)})
            </TabsTrigger>
            <TabsTrigger value="mods">
              Mods ({num(data?.mods.length ?? 0)})
            </TabsTrigger>
            <TabsTrigger value="arcanes">
              Arcanes ({num(data?.arcanes.length ?? 0)})
            </TabsTrigger>
            <TabsTrigger value="misc">
              Misc ({num(data?.misc.length ?? 0)})
            </TabsTrigger>
            <TabsTrigger value="sets">
              Sets ({num(data?.sets.length ?? 0)})
            </TabsTrigger>
          </TabsList>

          <TabsContent value={tab}>
            <InventoryTabView
              key={tab}
              tab={tab}
              entries={entries}
              flags={flags}
            />
          </TabsContent>
        </Tabs>
      </Section>
    </Page>
  );
}
