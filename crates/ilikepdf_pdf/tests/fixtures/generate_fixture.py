"""Regenerate the deterministic, project-owned PDF test fixture."""

from pathlib import Path

from reportlab.pdfgen import canvas


OUTPUT = Path(__file__).with_name("two_page.pdf")


def main() -> None:
    pdf = canvas.Canvas(
        str(OUTPUT),
        pagesize=(300, 200),
        invariant=1,
        pageCompression=0,
    )
    pdf.setTitle("ilikepdf deterministic rendering fixture")
    pdf.setAuthor("ilikepdf contributors")
    pdf.setFont("Helvetica", 18)
    pdf.drawString(30, 145, "ilikepdf page one")
    pdf.setFillColorRGB(0.70, 0.14, 0.09)
    pdf.rect(30, 45, 240, 70, fill=1, stroke=0)
    pdf.showPage()
    pdf.setPageSize((200, 300))
    pdf.setFont("Helvetica", 18)
    pdf.drawString(25, 245, "ilikepdf page two")
    pdf.circle(100, 135, 55, fill=0, stroke=1)
    pdf.save()


if __name__ == "__main__":
    main()
