import fs from "node:fs/promises";
import path from "node:path";
import sharp from "sharp";
import type { PipelineContext, PipelinePlugin, SupportedExtension } from "../types.js";
import { EncodeError, WriteError, PrismError } from "../types.js";

type SharpEncoder = (s: sharp.Sharp) => sharp.Sharp;

const encoders: Record<SupportedExtension, SharpEncoder> = {
    ".png":  (s) => s.png(),
    ".webp": (s) => s.webp({ lossless: true }),
    ".avif": (s) => s.avif({ lossless: true }),
};

export const encodeWritePlugin: PipelinePlugin = {
    name: "encode-write",

    async run(ctx: PipelineContext): Promise<void> {
        if (ctx.skipped || !ctx.pixels) return;

        const { pixels, width, height, outputPath } = ctx;
        const ext = path.extname(outputPath).toLowerCase() as SupportedExtension;
        const encode = encoders[ext];

        if (!encode) {
            throw new PrismError(`Unsupported output extension: ${ext}`);
        }

        let outputBuffer: Buffer;
        try {
            const sharpInstance = sharp(Buffer.from(pixels.buffer), {
                raw: { width, height, channels: 4 },
            });
            outputBuffer = await encode(sharpInstance).toBuffer();
        } catch (err) {
            throw new EncodeError(outputPath, err);
        }

        const tmpPath = `${outputPath}.prism.tmp`;
        try {
            await fs.mkdir(path.dirname(outputPath), { recursive: true });
            await fs.writeFile(tmpPath, outputBuffer);
            await fs.rename(tmpPath, outputPath);
        } catch (err) {
            // best-effort cleanup of the temp file on failure
            await fs.unlink(tmpPath).catch(() => undefined);
            throw new WriteError(outputPath, err);
        }
    },
};
