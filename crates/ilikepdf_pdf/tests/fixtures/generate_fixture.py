"""Regenerate the deterministic, project-owned PDF test fixtures."""

from pathlib import Path

from reportlab.pdfgen import canvas


ONE_PAGE_OUTPUT = Path(__file__).with_name("one_page.pdf")
TWO_PAGE_OUTPUT = Path(__file__).with_name("two_page.pdf")


def create_one_page_fixture() -> None:
    pdf = canvas.Canvas(
        str(ONE_PAGE_OUTPUT),
        pagesize=(300, 200),
        invariant=1,
        pageCompression=0,
    )
    pdf.setTitle("ilikepdf deterministic one-page fixture")
    pdf.setAuthor("ilikepdf contributors")
    pdf.setFont("Helvetica", 18)
    pdf.drawString(30, 145, "ilikepdf one-page PDF")
    pdf.setFillColorRGB(0.12, 0.45, 0.70)
    pdf.rect(30, 45, 240, 70, fill=1, stroke=0)
    pdf.save()


def create_two_page_fixture() -> None:
    pdf = canvas.Canvas(
        str(TWO_PAGE_OUTPUT),
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


def main() -> None:
    create_one_page_fixture()
    create_two_page_fixture()


if __name__ == "__main__":
    main()
