/**
 * A photo made safe to send: redrawn on a canvas before a byte leaves the phone, so EXIF (location
 * included) is gone, a HEIC arrives as JPEG, and `LONGEST` keeps it under the box's 5 MB ceiling.
 */

export const LONGEST = 2000;

const QUALITY = 0.85;

/** Unchanged when it fits, else scaled so its longest edge is `longest`, never below one pixel. */
export function fitWithin(
  width: number,
  height: number,
  longest: number = LONGEST,
): { width: number; height: number } {
  const edge = Math.max(width, height);
  if (edge <= longest) return { width, height };
  const scale = longest / edge;
  return {
    width: Math.max(1, Math.round(width * scale)),
    height: Math.max(1, Math.round(height * scale)),
  };
}

/** Base64 with no data-URL prefix, which is what `upload` sends. */
export async function reencode(file: Blob): Promise<string> {
  const bitmap = await createImageBitmap(file, { imageOrientation: "from-image" });
  const { width, height } = fitWithin(bitmap.width, bitmap.height);
  const canvas = document.createElement("canvas");
  canvas.width = width;
  canvas.height = height;
  const context = canvas.getContext("2d");
  if (context === null) throw new Error("this browser cannot draw the photo");
  context.drawImage(bitmap, 0, 0, width, height);
  bitmap.close();
  const blob = await new Promise<Blob>((resolve, reject) => {
    canvas.toBlob(
      (made) => (made === null ? reject(new Error("the photo could not be encoded")) : resolve(made)),
      "image/jpeg",
      QUALITY,
    );
  });
  return encoded(blob);
}

/** A file's own bytes in base64: a PDF goes up unredrawn. */
export async function encoded(file: Blob): Promise<string> {
  const bytes = new Uint8Array(await file.arrayBuffer());
  let binary = "";
  for (let at = 0; at < bytes.length; at += 0x8000) {
    binary += String.fromCharCode(...bytes.subarray(at, at + 0x8000));
  }
  return btoa(binary);
}
