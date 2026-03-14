export const SUPPORTED_EXTENSIONS = [".png", ".webp", ".avif"] as const;
export type SupportedExtension = (typeof SUPPORTED_EXTENSIONS)[number];

export function isSupportedExtension(ext: string): ext is SupportedExtension {
    return (SUPPORTED_EXTENSIONS as readonly string[]).includes(ext.toLowerCase());
}

/** Mutable context threaded through every pipeline stage. */
export interface PipelineContext {
    /** Absolute path of the source file. */
    inputPath: string;
    /** Absolute path where the result will be written. */
    outputPath: string;
    /** Raw RGBA pixel buffer (populated by the load stage). */
    pixels: Uint8ClampedArray | null;
    width: number;
    height: number;
    /** Number of transparent pixels that were replaced. */
    replacedCount: number;
    /** Whether the image actually needed any work. */
    skipped: boolean;
}

export function createContext(inputPath: string, outputPath: string): PipelineContext {
    return {
        inputPath,
        outputPath,
        pixels: null,
        width: 0,
        height: 0,
        replacedCount: 0,
        skipped: false,
    };
}

/** A single stage in the processing pipeline. */
export interface PipelinePlugin {
    name: string;
    run(ctx: PipelineContext): Promise<void>;
}

export type ProcessResult =
    | { ok: true; skipped: boolean; replacedCount: number; outputPath: string }
    | { ok: false; error: PrismError };

export class PrismError extends Error {
    constructor(
        message: string,
        public readonly cause?: unknown,
    ) {
        super(message);
        this.name = "PrismError";
    }
}

export class LoadError extends PrismError {
    constructor(path: string, cause?: unknown) {
        super(`Failed to load image: ${path}`, cause);
        this.name = "LoadError";
    }
}

export class EncodeError extends PrismError {
    constructor(path: string, cause?: unknown) {
        super(`Failed to encode output: ${path}`, cause);
        this.name = "EncodeError";
    }
}

export class WriteError extends PrismError {
    constructor(path: string, cause?: unknown) {
        super(`Failed to write file: ${path}`, cause);
        this.name = "WriteError";
    }
}