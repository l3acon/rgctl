import { GoogleAnalytics } from "@next/third-parties/google";

/** GA4 measurement ID from env (e.g. G-XXXXXXXX). Empty = analytics off. */
export function Analytics() {
  const gaId = process.env.NEXT_PUBLIC_GA_MEASUREMENT_ID?.trim();
  if (!gaId) return null;
  return <GoogleAnalytics gaId={gaId} />;
}
