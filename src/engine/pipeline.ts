import { loadPlugin } from "./plugins/load.js";
import { voronoiFillPlugin } from "./plugins/voronoiFill.js";
import { encodeWritePlugin } from "./plugins/encodeWrite.js";
import {
    createContext,
    PrismError,
    type PipelinePlugin,
    type ProcessResult,
} from "./types.js";

const DEFAULT_PIPELINE: PipelinePlugin[] = [
    loadPlugin,
    voronoiFillPlugin,
    encodeWritePlugin,
];

export interface ProcessOptions {
    /** Override the plugin pipeline (useful for testing). */
    pipeline?: PipelinePlugin[];
}

/**
 * Runs a file through the full Prism pipeline.
 *
 * @param inputPath  Absolute path to the source image.
 * @param outputPath Absolute path where the result should be written.
 * @returns          A typed `ProcessResult` — never throws.
 */
export async function processFile(
    inputPath: string,
    outputPath: string,
    options: ProcessOptions = {},
): Promise<ProcessResult> {
    const pipeline = options.pipeline ?? DEFAULT_PIPELINE;
    const ctx = createContext(inputPath, outputPath);

    try {
        for (const plugin of pipeline) {
            await plugin.run(ctx);

            // Short-circuit if a plugin marked the image as skippable
            if (ctx.skipped) break;
        }

        return {
            ok: true,
            skipped: ctx.skipped,
            replacedCount: ctx.replacedCount,
            outputPath: ctx.outputPath,
        };
    } catch (err) {
        const error =
            err instanceof PrismError
                ? err
                : new PrismError("Unexpected error during processing", err);

        return { ok: false, error };
    }
}

// re-export types so callers only need to import from engine/pipeline
export type { ProcessResult, PipelinePlugin, PipelineContext } from "./types.js";
export { PrismError, LoadError, EncodeError, WriteError } from "./types.js";