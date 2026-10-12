/** Matches Firebase Storage `validImageUpload`, so a stored image can be published. Copied from the web app. */
export const MAX_IMAGE_BYTES = 2 * 1024 * 1024;
export const ALLOWED_IMAGE_TYPES = new Set([
  "image/jpeg",
  "image/png",
  "image/webp",
]);

export function validateImageFile(file: File): string | null {
  if (!ALLOWED_IMAGE_TYPES.has(file.type)) {
    return "Unsupported format. Use JPEG, PNG, or WebP.";
  }
  if (file.size > MAX_IMAGE_BYTES) {
    return "Image too large. Maximum size is 2 MB.";
  }
  return null;
}

function loadImage(file: File): Promise<HTMLImageElement> {
  return new Promise((resolve, reject) => {
    const url = URL.createObjectURL(file);
    const img = new Image();
    img.onload = () => {
      URL.revokeObjectURL(url);
      resolve(img);
    };
    img.onerror = () => {
      URL.revokeObjectURL(url);
      reject(new Error("Could not read image file."));
    };
    img.src = url;
  });
}

function canvasToFile(
  canvas: HTMLCanvasElement,
  quality: number,
  filename: string,
): Promise<File> {
  return new Promise((resolve, reject) => {
    canvas.toBlob(
      (blob) => {
        if (!blob) {
          reject(new Error("Failed to compress image."));
          return;
        }
        resolve(new File([blob], filename, { type: "image/jpeg" }));
      },
      "image/jpeg",
      quality,
    );
  });
}

async function compressImageToLimit(file: File): Promise<File> {
  const img = await loadImage(file);
  const canvas = document.createElement("canvas");
  const ctx = canvas.getContext("2d");
  if (!ctx) throw new Error("Failed to compress image.");

  let width = img.naturalWidth;
  let height = img.naturalHeight;
  const maxDimension = 2048;
  if (Math.max(width, height) > maxDimension) {
    const scale = maxDimension / Math.max(width, height);
    width = Math.round(width * scale);
    height = Math.round(height * scale);
  }

  const baseName = file.name.replace(/\.[^.]+$/, "") || "image";
  const filename = `${baseName}.jpg`;

  const draw = (w: number, h: number) => {
    canvas.width = w;
    canvas.height = h;
    ctx.drawImage(img, 0, 0, w, h);
  };

  draw(width, height);

  for (let quality = 0.92; quality >= 0.5; quality -= 0.08) {
    const compressed = await canvasToFile(canvas, quality, filename);
    if (compressed.size <= MAX_IMAGE_BYTES) return compressed;
  }

  for (let scale = 0.85; scale >= 0.4; scale -= 0.15) {
    const w = Math.max(1, Math.round(width * scale));
    const h = Math.max(1, Math.round(height * scale));
    draw(w, h);
    const compressed = await canvasToFile(canvas, 0.85, filename);
    if (compressed.size <= MAX_IMAGE_BYTES) return compressed;
  }

  throw new Error(
    "Image too large. Maximum size is 2 MB — try a smaller image.",
  );
}

/**
 * Ensures a file satisfies Storage rules (type + size). Compresses when over 2 MB.
 */
export async function prepareImageForUpload(file: File): Promise<File> {
  if (!file.type.startsWith("image/")) {
    throw new Error("Unsupported format. Use JPEG, PNG, or WebP.");
  }

  const validationError = validateImageFile(file);
  if (!validationError) return file;

  if (validationError.includes("Unsupported format")) {
    throw new Error(validationError);
  }

  return compressImageToLimit(file);
}

