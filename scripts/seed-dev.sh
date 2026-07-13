#!/usr/bin/env bash
# Seed dev data — creates admin user + uploads test EPUB books with covers,
# and adds KOReader sync metadata for UI debugging.
# Run after `docker compose -f docker-compose.dev.yml up -d`
# Requires: curl, zip, python3

set -euo pipefail

BASE="${1:-http://localhost:3000}"
USERNAME="admin"
PASSWORD="password"

echo "==> Signing up admin user ..."
if ! curl -sf -X POST "$BASE/signup" \
	-H "Content-Type: application/x-www-form-urlencoded" \
	-d "username=$USERNAME&password=$PASSWORD" \
	-c /tmp/papyrd-cookies.txt >/dev/null 2>&1; then
	echo "     User exists, signing in instead ..."
	curl -sf -X POST "$BASE/signin" \
		-H "Content-Type: application/x-www-form-urlencoded" \
		-d "username=$USERNAME&password=$PASSWORD" \
		-c /tmp/papyrd-cookies.txt >/dev/null
fi

make_cover_png() {
	# Generate a gradient 200x266 PNG using Python builtins
	local out="$1" r1="$2" g1="$3" b1="$4" r2="$5" g2="$6" b2="$7"
	python3 -c "
import struct, zlib
def chunk(t, d):
    c = t + d
    crc = struct.pack('>I', zlib.crc32(c) & 0xFFFFFFFF)
    return struct.pack('>I', len(d)) + c + crc
w, h = 200, 266
raw = b''
for y in range(h):
    t = y / (h - 1) if h > 1 else 0
    raw += b'\x00'
    for x in range(w):
        s = x / (w - 1) if w > 1 else 0
        blend = (t + s) / 2
        r = int(($r1) + (($r2) - ($r1)) * blend)
        g = int(($g1) + (($g2) - ($g1)) * blend)
        b = int(($b1) + (($b2) - ($b1)) * blend)
        raw += bytes([r, g, b])
png = b'\x89PNG\r\n\x1a\n' + chunk(b'IHDR', struct.pack('>IIBBBBB', w, h, 8, 2, 0, 0, 0)) + chunk(b'IDAT', zlib.compress(raw)) + chunk(b'IEND', b'')
with open('$out', 'wb') as f:
    f.write(png)
"
}

make_epub() {
	local title="$1"
	local identifier="$2"
	local author="$3"
	local cover_png="$4" # path to cover PNG (optional, can be empty)

	local dir
	dir="$(mktemp -d)"
	echo -n "application/epub+zip" >"$dir/mimetype"
	mkdir -p "$dir/META-INF"
	cat >"$dir/META-INF/container.xml" <<XML
<?xml version="1.0" encoding="UTF-8"?>
<container version="1.0" xmlns="urn:oasis:names:tc:opendocument:xmlns:container">
  <rootfiles>
    <rootfile full-path="content.opf" media-type="application/oebps-package+xml"/>
  </rootfiles>
</container>
XML

	if [ -n "$cover_png" ] && [ -f "$cover_png" ]; then
		cp "$cover_png" "$dir/cover.png"
		cat >"$dir/content.opf" <<XML
<?xml version="1.0" encoding="UTF-8"?>
<package version="2.0" unique-identifier="bookid"
  xmlns="http://www.idpf.org/2007/opf">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>$title</dc:title>
    <dc:identifier id="bookid">$identifier</dc:identifier>
    <dc:creator opf:role="aut">$author</dc:creator>
    <meta name="cover" content="cover-image"/>
  </metadata>
  <manifest>
    <item id="cover-image" href="cover.png" media-type="image/png" properties="cover-image"/>
  </manifest>
  <spine/>
</package>
XML
		pushd "$dir" >/dev/null
		zip -q0X "$OLDPWD/$identifier.epub" mimetype
		zip -qr "$OLDPWD/$identifier.epub" META-INF content.opf cover.png
		popd >/dev/null
	else
		cat >"$dir/content.opf" <<XML
<?xml version="1.0" encoding="UTF-8"?>
<package version="2.0" unique-identifier="bookid"
  xmlns="http://www.idpf.org/2007/opf">
  <metadata xmlns:dc="http://purl.org/dc/elements/1.1/">
    <dc:title>$title</dc:title>
    <dc:identifier id="bookid">$identifier</dc:identifier>
    <dc:creator opf:role="aut">$author</dc:creator>
  </metadata>
</package>
XML
		pushd "$dir" >/dev/null
		zip -q0X "$OLDPWD/$identifier.epub" mimetype
		zip -qr "$OLDPWD/$identifier.epub" META-INF content.opf
		popd >/dev/null
	fi
	rm -rf "$dir"
}

echo "==> Generating cover images ..."
COVERS="$(mktemp -d)"

# ── Color palette for 40+ unique covers ──
make_cover_png "$COVERS/c01.png" 215 100 50 50 30 20   # rust orange → dark brown
make_cover_png "$COVERS/c02.png" 20 80 140 10 40 70    # deep blue → navy
make_cover_png "$COVERS/c03.png" 30 140 80 15 70 40    # emerald → pine
make_cover_png "$COVERS/c04.png" 180 60 50 90 30 25    # warm red → burgundy
make_cover_png "$COVERS/c05.png" 30 60 100 15 30 50    # steel blue → dark slate
make_cover_png "$COVERS/c06.png" 200 100 20 100 50 10  # amber → copper
make_cover_png "$COVERS/c07.png" 220 100 40 110 50 20  # gold → bronze
make_cover_png "$COVERS/c08.png" 80 30 120 40 15 60    # violet → plum
make_cover_png "$COVERS/c09.png" 50 50 50 25 25 25     # charcoal → near-black
make_cover_png "$COVERS/c10.png" 10 80 60 5 40 30      # teal → dark teal
make_cover_png "$COVERS/c11.png" 140 60 80 70 30 40    # mauve → wine
make_cover_png "$COVERS/c12.png" 200 160 60 100 80 30  # cream → mustard
make_cover_png "$COVERS/c13.png" 60 20 100 30 10 50    # indigo → midnight
make_cover_png "$COVERS/c14.png" 130 60 20 65 30 10    # terracotta → umber
make_cover_png "$COVERS/c15.png" 220 80 120 110 40 60  # coral → rose
make_cover_png "$COVERS/c16.png" 30 100 120 15 50 60   # ocean → deep teal
make_cover_png "$COVERS/c17.png" 180 140 80 90 70 40   # sand → tan
make_cover_png "$COVERS/c18.png" 90 30 30 45 15 15     # dark red → almost black
make_cover_png "$COVERS/c19.png" 40 120 200 20 60 100  # sky blue → royal
make_cover_png "$COVERS/c20.png" 70 140 50 35 70 25    # lime → olive
make_cover_png "$COVERS/c21.png" 160 100 120 80 50 60  # dusty rose → mauve
make_cover_png "$COVERS/c22.png" 40 40 40 20 20 20     # slate → dark gray
make_cover_png "$COVERS/c23.png" 200 50 80 100 25 40   # hot pink → crimson
make_cover_png "$COVERS/c24.png" 20 100 40 10 50 20    # forest → pine
make_cover_png "$COVERS/c25.png" 180 30 60 90 15 30    # scarlet → blood
make_cover_png "$COVERS/c26.png" 100 140 60 50 70 30   # sage → moss
make_cover_png "$COVERS/c27.png" 50 100 180 25 50 90   # cornflower → navy
make_cover_png "$COVERS/c28.png" 220 180 100 110 90 50 # parchment → gold
make_cover_png "$COVERS/c29.png" 80 20 20 40 10 10     # brick → mahogany
make_cover_png "$COVERS/c30.png" 120 30 150 60 15 75   # purple → eggplant
make_cover_png "$COVERS/c31.png" 30 80 30 15 40 15     # dark green → pine
make_cover_png "$COVERS/c32.png" 200 120 40 100 60 20  # tangerine → rust
make_cover_png "$COVERS/c33.png" 60 60 100 30 30 50    # periwinkle → twilight
make_cover_png "$COVERS/c34.png" 140 40 60 70 20 30    # cherry → maroon
make_cover_png "$COVERS/c35.png" 20 140 140 10 70 70   # cyan → teal
make_cover_png "$COVERS/c36.png" 160 160 40 80 80 20   # yellow → olive
make_cover_png "$COVERS/c37.png" 90 50 100 45 25 50    # lavender → plum
make_cover_png "$COVERS/c38.png" 40 50 30 20 25 15     # olive drab → army
make_cover_png "$COVERS/c39.png" 200 60 60 100 30 30   # tomato → brick
make_cover_png "$COVERS/c40.png" 70 30 90 35 15 45     # amethyst → grape

echo "==> Creating test EPUBs ..."
TMPDIR="$(mktemp -d)"
pushd "$TMPDIR" >/dev/null

# ── Book catalogue ──
# Format: make_epub "Title" "urn:isbn:..." "Author Name" "$COVERS/cXX.png"
# Some books use "" for cover to test fallback gradient.

# ═══ Isaac Asimov — 8 books (prolific) ═══
make_epub "Foundation" "urn:seed:asimov-01" "Isaac Asimov" "$COVERS/c01.png"
make_epub "Foundation and Empire" "urn:seed:asimov-02" "Isaac Asimov" "$COVERS/c02.png"
make_epub "Second Foundation" "urn:seed:asimov-03" "Isaac Asimov" "$COVERS/c03.png"
make_epub "I, Robot" "urn:seed:asimov-04" "Isaac Asimov" "$COVERS/c04.png"
make_epub "The Caves of Steel" "urn:seed:asimov-05" "Isaac Asimov" "$COVERS/c05.png"
make_epub "The Naked Sun" "urn:seed:asimov-06" "Isaac Asimov" "$COVERS/c06.png"
make_epub "The Robots of Dawn" "urn:seed:asimov-07" "Isaac Asimov" "$COVERS/c07.png"
make_epub "Robots and Empire" "urn:seed:asimov-08" "Isaac Asimov" "$COVERS/c08.png"

# ═══ Robert C. Martin — 6 books (prolific) ═══
make_epub "Clean Code" "urn:seed:martin-01" "Robert C. Martin" "$COVERS/c09.png"
make_epub "The Clean Coder" "urn:seed:martin-02" "Robert C. Martin" "$COVERS/c10.png"
make_epub "Clean Architecture" "urn:seed:martin-03" "Robert C. Martin" "$COVERS/c11.png"
make_epub "Clean Agile" "urn:seed:martin-04" "Robert C. Martin" "$COVERS/c12.png"
make_epub "Clean Craftsmanship" "urn:seed:martin-05" "Robert C. Martin" "$COVERS/c13.png"
make_epub "UML for Java Programmers" "urn:seed:martin-06" "Robert C. Martin" "$COVERS/c14.png"

# ═══ J.R.R. Tolkien — 5 books (mid-high) ═══
make_epub "The Hobbit" "urn:seed:tolkien-01" "J.R.R. Tolkien" "$COVERS/c15.png"
make_epub "The Fellowship of the Ring" "urn:seed:tolkien-02" "J.R.R. Tolkien" "$COVERS/c16.png"
make_epub "The Two Towers" "urn:seed:tolkien-03" "J.R.R. Tolkien" "$COVERS/c17.png"
make_epub "The Return of the King" "urn:seed:tolkien-04" "J.R.R. Tolkien" "$COVERS/c18.png"
make_epub "The Silmarillion" "urn:seed:tolkien-05" "J.R.R. Tolkien" "$COVERS/c19.png"

# ═══ Cal Newport — 3 books (mid) ═══
make_epub "Deep Work" "urn:seed:newport-01" "Cal Newport" "$COVERS/c20.png"
make_epub "Digital Minimalism" "urn:seed:newport-02" "Cal Newport" "$COVERS/c21.png"
make_epub "So Good They Can't Ignore You" "urn:seed:newport-03" "Cal Newport" "$COVERS/c22.png"

# ═══ Martin Kleppmann — 3 books (mid) ═══
make_epub "Designing Data-Intensive Applications" "urn:seed:kleppmann-01" "Martin Kleppmann" "$COVERS/c23.png"
make_epub "Stream Processing with Apache Kafka" "urn:seed:kleppmann-02" "Martin Kleppmann" "$COVERS/c24.png"
make_epub "Distributed Systems Observability" "urn:seed:kleppmann-03" "Martin Kleppmann" "$COVERS/c25.png"

# ═══ Gene Kim — 3 books (mid) ═══
make_epub "The Phoenix Project" "urn:seed:kim-01" "Gene Kim" "$COVERS/c26.png"
make_epub "The DevOps Handbook" "urn:seed:kim-02" "Gene Kim" "$COVERS/c27.png"
make_epub "The Unicorn Project" "urn:seed:kim-03" "Gene Kim" "$COVERS/c28.png"

# ═══ Steve Klabnik — 2 books (few) ═══
make_epub "The Rust Programming Language" "urn:seed:klabnik-01" "Steve Klabnik" "$COVERS/c29.png"
make_epub "Rust for Rustaceans" "urn:seed:klabnik-02" "Steve Klabnik" "$COVERS/c30.png"

# ═══ Peter Thiel — 2 books (few) ═══
make_epub "Zero to One" "urn:seed:thiel-01" "Peter Thiel" "$COVERS/c31.png"
make_epub "The Diversity Myth" "urn:seed:thiel-02" "Peter Thiel" "$COVERS/c32.png"

# ═══ Betsy Beyer — 2 books (few) ═══
make_epub "Site Reliability Engineering" "urn:seed:beyer-01" "Betsy Beyer" "$COVERS/c33.png"
make_epub "The Site Reliability Workbook" "urn:seed:beyer-02" "Betsy Beyer" "$COVERS/c34.png"

# ═══ Dave Thomas — 2 books (few) ═══
make_epub "The Pragmatic Programmer" "urn:seed:thomas-01" "Dave Thomas" "$COVERS/c35.png"
make_epub "Programming Ruby" "urn:seed:thomas-02" "Dave Thomas" "$COVERS/c36.png"

# ═══ Douglas Adams — 2 books (few) ═══
make_epub "The Hitchhiker's Guide to the Galaxy" "urn:seed:adams-01" "Douglas Adams" "$COVERS/c37.png"
make_epub "The Restaurant at the End of the Universe" "urn:seed:adams-02" "Douglas Adams" "$COVERS/c38.png"

# ═══ Single-book authors ═══
make_epub "To Kill a Mockingbird" "urn:seed:lee-01" "Harper Lee" "$COVERS/c39.png"
make_epub "Moby Dick" "urn:seed:melville-01" "Herman Melville" "$COVERS/c40.png"
make_epub "The Handmaid's Tale" "urn:seed:atwood-01" "Margaret Atwood" "" # no cover — fallback
make_epub "Nineteen Eighty-Four" "urn:seed:orwell-01" "George Orwell" ""  # no cover — fallback
make_epub "Brave New World" "urn:seed:huxley-01" "Aldous Huxley" ""       # no cover — fallback

echo "==> Uploading test EPUBs (skipping duplicates) ..."
for epub in *.epub; do
	echo "     $epub"
	curl -s -X POST "$BASE/upload" \
		-F "epubs=@$epub;type=application/epub+zip" \
		-b /tmp/papyrd-cookies.txt >/dev/null
done

popd >/dev/null
rm -rf "$TMPDIR" "$COVERS"

echo "==> Seeding KOReader sync progress ..."
docker compose -f docker-compose.dev.yml exec -T postgres psql -U papyrd -d papyrd <<'SQL'
WITH admin AS (
  SELECT id FROM users WHERE username = 'admin'
)
INSERT INTO reading_progress (user_id, document, progress, percentage, device, device_id, updated_at)
SELECT a.id, e.partial_md5, v.progress, v.percentage, v.device, v.device_id, v.updated_at
FROM admin a
CROSS JOIN (VALUES
  ('Foundation',                                    '/body/DocFragment[1]/body/div/p[380]/text().12',  1.0,  'KOReader (Kobo Libra 2)',   'kobo-libra2-001',   NOW() - INTERVAL '2 days'),
  ('Deep Work',                                     '/body/DocFragment[1]/body/div/p[220]/text().45',  1.0,  'KOReader (Android)',         'android-phone-001',  NOW() - INTERVAL '5 days'),
  ('I, Robot',                                      '/body/DocFragment[1]/body/div/p[260]/text().80',  0.75, 'KOReader (Kobo Libra 2)',   'kobo-libra2-001',   NOW() - INTERVAL '1 day'),
  ('Clean Code',                                    '/body/DocFragment[1]/body/div/p[180]/text().33',  0.50, 'KOReader (Desktop)',         'desktop-linux-001',  NOW() - INTERVAL '3 days'),
  ('The Hobbit',                                    '/body/DocFragment[1]/body/div/p[95]/text().67',   0.30, 'KOReader (Kobo Libra 2)',   'kobo-libra2-001',   NOW() - INTERVAL '1 hour'),
  ('The Pragmatic Programmer',                      '/body/DocFragment[1]/body/div/p[40]/text().10',   0.10, 'KOReader (Android)',         'android-phone-001',  NOW() - INTERVAL '7 days'),
  ('Designing Data-Intensive Applications',         '/body/DocFragment[1]/body/div/p[340]/text().55',  0.60, 'KOReader (Kobo Libra 2)',   'kobo-libra2-001',   NOW() - INTERVAL '12 hours'),
  ('Site Reliability Engineering',                  '/body/DocFragment[1]/body/div/p[420]/text().91',  0.85, 'KOReader (Android Tablet)',  'android-tablet-001', NOW() - INTERVAL '6 hours'),
  ('Foundation and Empire',                         '/body/DocFragment[1]/body/div/p[150]/text().22',  0.42, 'KOReader (Kobo Libra 2)',   'kobo-libra2-001',   NOW() - INTERVAL '3 days'),
  ('The Phoenix Project',                           '/body/DocFragment[1]/body/div/p[18]/text().5',    0.05, 'KOReader (Desktop)',         'desktop-mac-001',    NOW() - INTERVAL '30 days'),
  ('Zero to One',                                   '/body/DocFragment[1]/body/div/p[8]/text().3',     0.02, 'KOReader (Kobo Libra 2)',   'kobo-libra2-001',   NOW() - INTERVAL '14 days'),
  ('The Caves of Steel',                            '/body/DocFragment[1]/body/div/p[55]/text().88',   0.15, 'KOReader (Kobo Libra 2)',   'kobo-libra2-001',   NOW() - INTERVAL '10 minutes')
) AS v(title, progress, percentage, device, device_id, updated_at)
JOIN publications p ON p.title = v.title
JOIN assets e ON e.publication_id = p.id AND e.kind = 'primary_epub' AND e.partial_md5 IS NOT NULL
ON CONFLICT (user_id, document) DO UPDATE SET
  progress   = EXCLUDED.progress,
  percentage  = EXCLUDED.percentage,
  device      = EXCLUDED.device,
  device_id   = EXCLUDED.device_id,
  updated_at  = EXCLUDED.updated_at;
SQL
echo "     Done."

echo "==> Seed complete! Open $BASE in your browser."
echo "     Login: $USERNAME / $PASSWORD"
rm -f /tmp/papyrd-cookies.txt
