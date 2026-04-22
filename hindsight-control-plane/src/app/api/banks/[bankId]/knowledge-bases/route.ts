import { NextResponse } from "next/server";
import { dataplaneBankUrl, getDataplaneHeaders } from "@/lib/hindsight-client";

export async function GET(request: Request, { params }: { params: Promise<{ bankId: string }> }) {
  try {
    const { bankId } = await params;
    const url = dataplaneBankUrl(bankId, "/knowledge-bases");
    const response = await fetch(url, { method: "GET", headers: getDataplaneHeaders() });

    if (!response.ok) {
      const errorText = await response.text();
      console.error("API error listing knowledge bases:", errorText);
      return NextResponse.json(
        { error: "Failed to list knowledge bases" },
        { status: response.status }
      );
    }

    const data = await response.json();
    return NextResponse.json(data, { status: 200 });
  } catch (error) {
    console.error("Error listing knowledge bases:", error);
    return NextResponse.json({ error: "Failed to list knowledge bases" }, { status: 500 });
  }
}
