import { useState } from "react";
import {
  CartesianGrid,
  LineChart as Chart,
  Line,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from "recharts";
import { ChartHeader, readingsTooltip } from "@/components/chart-hover";
import { OptionSelect } from "@/components/option-select";
import {
  ARBITRATION_RUN_METRIC_OPTIONS,
  ARBITRATION_RUN_METRICS,
  type ArbitrationRunMetric,
  type ArbitrationRunReading,
  clockText,
  metricReadings,
} from "@/lib/arbitration-runs";
import { AXIS_TICK, GRID, LABELLED_MARGIN, LINE_CURSOR } from "@/lib/chart";
import type { ArbitrationRun } from "@/types";

const COLOR = "var(--primary)";
const AXIS_WIDTH = 40;

type KnownReading = ArbitrationRunReading & { value: number };

export function ArbitrationRunChart({ run }: { run: ArbitrationRun }) {
  const [metric, setMetric] = useState<ArbitrationRunMetric>("drone_rate");
  const { label, digits, headline } = ARBITRATION_RUN_METRICS[metric];
  const format = (value: number) => value.toFixed(digits);
  const readings = metricReadings(run, metric);
  const known = readings.filter(
    (reading): reading is KnownReading => reading.value !== null,
  );
  const shown =
    headline === "latest"
      ? known.at(-1)
      : known.reduce<KnownReading | undefined>(
          (best, reading) =>
            best !== undefined && best.value >= reading.value ? best : reading,
          undefined,
        );

  return (
    <div className="flex flex-col gap-2">
      <ChartHeader
        color={COLOR}
        label={label}
        value={shown ? format(shown.value) : "-"}
        caption={
          shown
            ? `${headline === "peak" ? "peak at" : "at"} ${shown.label}`
            : "less than 30 s sampled"
        }
        control={
          <OptionSelect
            label="Measurement"
            value={metric}
            options={ARBITRATION_RUN_METRIC_OPTIONS}
            onChange={setMetric}
            size="sm"
            dense
          />
        }
      />
      <div className="h-40">
        <ResponsiveContainer>
          <Chart data={readings} margin={LABELLED_MARGIN}>
            <CartesianGrid vertical={false} {...GRID} />
            <XAxis
              dataKey="secs"
              type="number"
              domain={[0, run.duration_secs]}
              tickFormatter={clockText}
              tick={AXIS_TICK}
              tickLine={false}
              axisLine={false}
              minTickGap={32}
            />
            <YAxis
              width={AXIS_WIDTH}
              domain={[0, "dataMax"]}
              padding={{ top: 4 }}
              tickCount={3}
              tick={AXIS_TICK}
              tickFormatter={format}
              tickLine={false}
              axisLine={false}
            />
            {run.rotations.map((rotation) => (
              <ReferenceLine
                key={rotation.index}
                x={rotation.at_secs}
                stroke="currentColor"
                strokeOpacity={0.25}
                strokeDasharray="3 3"
                label={{
                  value: rotation.index,
                  position: "top",
                  ...AXIS_TICK,
                }}
              />
            ))}
            <Tooltip
              cursor={LINE_CURSOR}
              isAnimationActive={false}
              content={readingsTooltip<ArbitrationRunReading>((reading) => [
                {
                  key: "value",
                  color: COLOR,
                  text: reading.value === null ? "-" : format(reading.value),
                },
              ])}
            />
            <Line
              dataKey="value"
              type="monotone"
              stroke={COLOR}
              strokeWidth={2}
              connectNulls
              dot={false}
              activeDot={{
                r: 4,
                fill: COLOR,
                stroke: "var(--card)",
                strokeWidth: 2,
              }}
              isAnimationActive={false}
            />
          </Chart>
        </ResponsiveContainer>
      </div>
    </div>
  );
}
