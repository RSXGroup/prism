import path from "node:path";
import pLimit from "p-limit";
import cliProgress from "cli-progress";
import { consola } from "consola";
import { processFile, type ProcessResult } from "../engine/index.js";
import { resolveOutputPath } from "./output.js";

export interface BatchOptions {
    /** Max number of files processed simultaneously (default: CPU count or 4). */
    concurrency?: number;
    /** Suffix appended to the output file stem (default: `"fixed"`). */
    suffix?: string;
}

export interface BatchSummary {
    total: number;
    succeeded: number;
    skipped: number;
    failed: number;
    results: Array<{ inputPath: string; result: ProcessResult }>;
}

/**
 * Processes a batch of files concurrently, showing a live progress bar.
 */
export async function processBatch(
    targets: string[],
    options: BatchOptions = {},
): Promise<BatchSummary> {
    const concurrency = options.concurrency ?? Math.max(1, navigator?.hardwareConcurrency ?? 4);
    const suffix = options.suffix ?? "fixed";

    const limit = pLimit(concurrency);
    const summary: BatchSummary = {
        total: targets.length,
        succeeded: 0,
        skipped: 0,
        failed: 0,
        results: [],
    };

    const bar = new cliProgress.SingleBar(
        {
            format: "  {bar} {percentage}% | {value}/{total} | {filename}",
            barCompleteChar: "█",
            barIncompleteChar: "░",
            hideCursor: true,
            clearOnComplete: false,
            stopOnComplete: true,
        },
        cliProgress.Presets.shades_classic,
    );

    bar.start(targets.length, 0, { filename: "starting…" });

    const tasks = targets.map((inputPath) =>
        limit(async () => {
            const outputPath = resolveOutputPath(inputPath, suffix);
            const result = await processFile(inputPath, outputPath);

            bar.increment(1, { filename: path.basename(inputPath) });
            summary.results.push({ inputPath, result });

            if (result.ok) {
                if (result.skipped) {
                    summary.skipped++;
                } else {
                    summary.succeeded++;
                }
            } else {
                summary.failed++;
            }
        }),
    );

    await Promise.allSettled(tasks);
    bar.stop();

    for (const { inputPath, result } of summary.results) {
        const name = path.basename(inputPath);
        if (!result.ok) {
            consola.error(`  ${name}: ${result.error.message}`);
        } else if (result.skipped) {
            consola.warn(`  ${name}: no transparent edges — skipped`);
        } else {
            consola.success(
                `  ${name} → ${path.basename(result.outputPath)} ` +
                `(${result.replacedCount.toLocaleString()} px replaced)`,
            );
        }
    }

    return summary;
}
