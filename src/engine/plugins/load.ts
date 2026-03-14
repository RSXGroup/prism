import sharp from "sharp";
import type { PipelineContext, PipelinePlugin } from "../types.js";
import { LoadError } from "../types.js";

export const loadPlugin: PipelinePlugin = {
    name: "load",

    async run(ctx: PipelineContext): Promise<void> {
        try {
            const image = sharp(ctx.inputPath, { failOn: "none" }).ensureAlpha();
            const metadata = await image.metadata();

            if (!metadata.width || !metadata.height) {
                throw new LoadError(ctx.inputPath, new Error("Unable to read image dimensions"));
            }

            ctx.width = metadata.width;
            ctx.height = metadata.height;

            const raw = await image.raw().toBuffer();
            ctx.pixels = new Uint8ClampedArray(raw);
        } catch (err) {
            if (err instanceof LoadError) throw err;
            throw new LoadError(ctx.inputPath, err);
        }
    },
};
