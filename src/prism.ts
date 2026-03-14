#!/usr/bin/env node
import { Command } from "commander";
import path from "node:path";
import { consola } from "consola";
import { SUPPORTED_EXTENSIONS } from "./engine/index.js";
import { collectFiles, validateFile } from "./cli/collect.js";
import { processBatch } from "./cli/batch.js";

const program = new Command();

program
    .name("prism")
    .description("Probably Repairs Inconsistent Semi-transparency")
    .version("1.2.0")
    .option("-f, --folder <path>", "Process all supported images in a folder")
    .option("-r, --recursive", "Scan folders recursively (requires -f)", false)
    .option(
        "-s, --suffix <suffix>",
        "Suffix appended to processed filenames before the extension",
        "fixed",
    )
    .option(
        "-c, --concurrency <n>",
        "Number of files to process simultaneously",
        (v) => {
            const n = parseInt(v, 10);
            if (isNaN(n) || n < 1) throw new Error("--concurrency must be a positive integer");
            return n;
        },
    )
    .argument("[files...]", `Images to process (${SUPPORTED_EXTENSIONS.join(", ")})`)
    .action(async (files: string[], options) => {
        consola.start(`prism v${program.version()}`);
        consola.info(`Supported formats: ${SUPPORTED_EXTENSIONS.join(", ")}`);

        let targets: string[] = [];

        try {
            if (options.folder) {
                const folderPath = path.resolve(options.folder as string);
                consola.info(
                    options.recursive
                        ? `Scanning recursively: ${folderPath}`
                        : `Scanning: ${folderPath}`,
                );
                targets = await collectFiles(folderPath, options.recursive as boolean);

                if (targets.length === 0) {
                    consola.warn("No supported image files found in the specified folder.");
                    process.exitCode = 0;
                    return;
                }
            } else if (files.length > 0) {
                if (options.recursive) {
                    consola.warn("--recursive is ignored when not using --folder.");
                }

                for (const file of files) {
                    const resolved = await validateFile(file);
                    if (resolved) {
                        targets.push(resolved);
                    } else {
                        consola.warn(`Skipping unsupported or missing file: ${file}`);
                    }
                }

                if (targets.length === 0) {
                    consola.warn("No valid image files provided.");
                    process.exitCode = 0;
                    return;
                }
            } else {
                consola.error("No input provided. Use --folder or specify files directly.");
                program.help({ error: true });
                return;
            }

            consola.info(`Processing ${targets.length} file(s) with suffix ".${options.suffix}"…`);

            const summary = await processBatch(targets, {
                suffix: options.suffix as string,
                concurrency: options.concurrency as number | undefined,
            });

            consola.box({
                title: "Summary",
                message: [
                    `Total:    ${summary.total}`,
                    `Fixed:    ${summary.succeeded}`,
                    `Skipped:  ${summary.skipped}`,
                    `Failed:   ${summary.failed}`,
                ].join("\n"),
            });

            process.exitCode = summary.failed > 0 ? 1 : 0;

        } catch (err: unknown) {
            const message = err instanceof Error ? err.message : String(err);
            consola.error(`Fatal: ${message}`);
            process.exit(1);
        }
    });

program.parse(process.argv);