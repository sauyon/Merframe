import { getVersion } from "@tauri-apps/api/app";
import { check } from "@tauri-apps/plugin-updater";
import { ExternalLink, FolderOpen, RefreshCw } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import Merframe from "@/components/icons/merframe";
import { Page, Section } from "@/components/page";
import { Badge } from "@/components/ui/badge";
import { Button } from "@/components/ui/button";
import { Hint } from "@/components/ui/hint";
import { api, openExternal, reportError } from "@/lib/bridge";
import { ago } from "@/lib/format";
import { notify } from "@/lib/toast";
import { useAppStore } from "@/stores/app-store";

interface AboutLink {
  label: string;
  url: string;
}

const PROJECT_LINKS: AboutLink[] = [
  { label: "yareli.net/merframe", url: "https://yareli.net/merframe" },
  { label: "GitHub", url: "https://github.com/akshualy/Merframe" },
];

const LEGAL_LINKS: AboutLink[] = [
  { label: "Legal Notice", url: "https://yareli.net/impressum" },
  { label: "Privacy", url: "https://yareli.net/privacy" },
  { label: "Terms", url: "https://yareli.net/terms" },
];

const CREDIT_LINKS: AboutLink[] = [
  { label: "AlecaFrame", url: "https://alecaframe.com" },
  { label: "warframe.market", url: "https://warframe.market" },
  {
    label: "WFCD",
    url: "https://github.com/WFCD",
  },
  {
    label: "Public Export Plus",
    url: "https://github.com/calamity-inc/warframe-public-export-plus",
  },
  { label: "Warframe wiki", url: "https://wiki.warframe.com" },
];

function AboutLinkButton({ link }: { link: AboutLink }) {
  return (
    <Button variant="outline" size="sm" onClick={() => openExternal(link.url)}>
      {link.label}
      <ExternalLink className="size-3.5" />
    </Button>
  );
}

function AboutLinkSection({
  title,
  description,
  links,
}: {
  title: string;
  description: string;
  links: AboutLink[];
}) {
  return (
    <Section title={title}>
      <p className="text-muted-foreground mb-3 text-sm">{description}</p>
      <div className="flex flex-wrap gap-2">
        {links.map((link) => (
          <AboutLinkButton key={link.url} link={link} />
        ))}
      </div>
    </Section>
  );
}

export function AboutPage() {
  const status = useAppStore((state) => state.status);
  const setUpdate = useAppStore((state) => state.setUpdate);
  const [version, setVersion] = useState("");
  const [logFile, setLogFile] = useState("");
  const [checking, setChecking] = useState(false);
  const [updatable, setUpdatable] = useState(false);

  useEffect(() => {
    getVersion().then(setVersion, reportError);
    api.updatesSupported().then(setUpdatable, reportError);
    api.appLogFile().then(setLogFile, reportError);
  }, []);

  const handleOpenDataFolder = useCallback(async () => {
    try {
      await api.openDataFolder();
    } catch (error) {
      reportError(error);
    }
  }, []);

  const handleCheck = useCallback(async () => {
    setChecking(true);
    try {
      const update = await check();
      if (update) {
        setUpdate(update);
      } else {
        notify.success("Merframe is up to date");
      }
    } catch (error) {
      reportError(error);
    } finally {
      setChecking(false);
    }
  }, [setUpdate]);

  return (
    <Page title="About" quote="about">
      <Section>
        <div className="flex flex-col gap-6 @sm:flex-row @sm:items-center">
          <Merframe className="text-primary size-24 shrink-0" />
          <div className="flex flex-col gap-3 text-sm">
            <p className="text-muted-foreground max-w-2xl">
              Merframe is an open-source alternative to AlecaFrame that runs
              natively on Linux (Proton) and Windows. It reads the running game
              and EE.log. Nothing is uploaded, snapshots stay in a local SQLite
              database.
            </p>
            <p className="text-muted-foreground max-w-2xl">
              Free software under the GNU General Public License, version 3 or
              later. No warranty. Merframe is a fan project and is not
              affiliated with Digital Extremes.
            </p>
            <div className="flex flex-wrap items-center gap-2">
              <Badge variant="secondary">Version {version}</Badge>
              {updatable && (
                <Button
                  variant="outline"
                  size="sm"
                  pending={checking}
                  onClick={handleCheck}
                >
                  <RefreshCw className="size-3.5" />
                  Check for Updates
                </Button>
              )}
              {PROJECT_LINKS.map((link) => (
                <AboutLinkButton key={link.url} link={link} />
              ))}
            </div>
          </div>
        </div>
      </Section>

      <Section title="Diagnostics">
        <dl className="grid gap-3 @sm:grid-cols-2">
          <div className="flex flex-col">
            <Hint as="dt">Game process</Hint>
            <dd className="text-sm">
              {status?.game_detected
                ? `running (pid ${status.pid ?? "?"})`
                : "not running"}
            </dd>
          </div>
          <div className="flex flex-col">
            <Hint as="dt">Last scan</Hint>
            <dd className="text-sm">{ago(status?.last_scan_at)}</dd>
          </div>
          <div className="flex flex-col">
            <Hint as="dt">World state</Hint>
            <dd className="text-sm">{ago(status?.world_state_at)}</dd>
          </div>
          <div className="flex flex-col @sm:col-span-2">
            <Hint as="dt">Merframe log</Hint>
            <dd className="text-sm break-all">{logFile}</dd>
          </div>
        </dl>
        <div className="mt-4">
          <Button variant="outline" size="sm" onClick={handleOpenDataFolder}>
            <FolderOpen className="size-3.5" />
            Open Merframe Data Folder
          </Button>
        </div>
      </Section>

      <AboutLinkSection
        title="Legal"
        description="Got questions about Privacy or Terms of Service? Everything can be found here."
        links={LEGAL_LINKS}
      />

      <AboutLinkSection
        title="Credits"
        description="Merframe could not exist without the below inspirations and sources."
        links={CREDIT_LINKS}
      />
    </Page>
  );
}
