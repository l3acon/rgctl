import { redirect } from "next/navigation";

/** Docs hub removed from nav — Guides is the primary entry. */
export default function DocsPage() {
  redirect("/docs/guides/");
}
