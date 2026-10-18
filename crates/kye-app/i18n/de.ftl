# KnowYourEgk UI-Texte - Deutsch.

# Name of this language in this language, shown in the language menu.
language-name = Deutsch

# Dates are stored as YYYYMMDD on the card.
date-format = { $day }.{ $month }.{ $year }

## Kopfzeile

header-tagline = Was auf deiner Gesundheitskarte gespeichert ist
header-offline = Offline · nichts wird gespeichert
language-label = Sprache
theme-label = Darstellung
theme-system = System
theme-light = Hell
theme-dark = Dunkel

## Leserleiste

readers-heading = Kartenleser
readers-none = Kein Kartenleser gefunden.
readers-unavailable = Chipkartendienst (PC/SC) nicht verfügbar: { $detail }
readers-footer = Kartenleser können jederzeit an- und abgesteckt werden. Ein Dual-Interface-Leser erscheint als zwei Slots.
slot-kind-contact = Kontakt-Slot
slot-kind-contactless = Kontaktlos-Slot (NFC)
slot-kind-unknown = Kartenleser-Slot
status-no-card = Keine Karte
status-inserted = Karte gesteckt
status-on-reader = Karte aufgelegt
status-unknown-card = Unbekannte Karte
can-label = Zugangsnummer (CAN)
can-where = 6 Ziffern, oben rechts auf der Vorderseite deiner Karte. Nicht deine PIN.
can-illustration = Die CAN steht oben rechts auf der Vorderseite der Karte.
can-note = Wird einmal für die verschlüsselte Verbindung genutzt und nie gespeichert.
button-read = Karte lesen
button-enter-can = 6-stellige CAN eingeben
button-reading = Lese …

## Startbildschirm

empty-heading = Sieh nach, was auf deiner Gesundheitskarte steht
empty-text = Stecke deine eGK in einen Kartenleser oder lege sie auf einen NFC-Leser. Klicke dann in der Leserliste auf „Karte lesen“.
empty-contact-title = Gesteckt (kontaktbehaftet)
empty-contact-text = Chip voran, wie am Leser markiert. Keine CAN nötig.
empty-nfc-title = Aufgelegt (kontaktlos)
empty-nfc-text = Braucht die 6-stellige CAN von der Kartenvorderseite. Die Verbindung ist verschlüsselt.

## Lesen

reading-heading = Deine Karte wird gelesen
reading-hint-secure = Kontaktlos und verschlüsselt. Das dauert etwa zwei Sekunden.
reading-hint-contact = Über den Kontakt-Slot.
reading-keep = Lass die Karte liegen, bis deine Daten angezeigt werden.
step-connecting = Verbinde mit der Karte
step-pace = Sicherer Kanal mit deiner CAN (PACE)
step-reading = Lese Versichertendaten
step-reading-secure = Lese Versichertendaten (verschlüsselt)
step-parsing = Werte Daten aus

## Fehler

error-can-required-title = Die CAN fehlt
error-can-required-text = Diese Karte lässt sich nur mit ihrer 6-stelligen Zugangsnummer (CAN) lesen. Gib sie in der Leserliste ein und versuche es erneut.
error-wrong-can-title = Die CAN passt nicht
error-wrong-can-text = Die Karte hat die Zugangsnummer abgelehnt. Vergleiche sie mit den 6 Ziffern oben rechts auf der Kartenvorderseite und versuche es erneut.
error-card-removed-title = Die Karte wurde bewegt
error-card-removed-text = Die Verbindung ist beim Lesen abgebrochen. Lege die Karte zurück und lass sie liegen, bis deine Daten angezeigt werden.
error-reader-gone-title = Der Kartenleser ist weg
error-reader-gone-text = Der Leser wurde abgesteckt oder ist nicht mehr verfügbar. Schließe ihn wieder an und versuche es erneut.
error-no-card-title = Keine Karte gefunden
error-no-card-text = In diesem Leser steckt keine Karte. Stecke die Karte ein oder lege sie auf.
error-unsupported-title = Diese Karte wird nicht unterstützt
error-unsupported-text = Die Karte bietet kein bekanntes Verfahren für den sicheren Kanal (PACE). KnowYourEgk liest eGKs der Generation G2.1.
error-service-title = Chipkartendienst nicht verfügbar
error-service-text = Der Chipkartendienst (PC/SC) deines Systems ist nicht erreichbar. Unter Linux muss pcscd laufen.
error-card-title = Lesen fehlgeschlagen
error-card-text = Die Karte hat unerwartet geantwortet: { $detail }
error-retry-can = CAN neu eingeben
error-back = Zurück zum Start

## Ergebnis

update-incomplete-title = Die letzte Aktualisierung dieser Daten wurde nicht abgeschlossen.
update-incomplete-text = Die Daten können unvollständig oder veraltet sein. Deine Arztpraxis kann die Karte beim nächsten Besuch aktualisieren.
insurant-id-inline = Versicherten-ID
via-contactless = Kontaktlos gelesen · verschlüsselt
via-contact-pace = Kontaktbehaftet gelesen · verschlüsselt
via-contact = Kontaktbehaftet gelesen
button-discard = Daten verwerfen
tab-overview = Übersicht
tab-personal = Persönliche Daten
tab-insurance = Versicherung
tab-raw = Alle Felder
section-personal = Persönliche Daten
section-insurance = Versicherung
raw-intro = Alle Elemente der XML-Dokumente auf der Karte, auch Felder, für die diese App keine Bezeichnung hat.
raw-col-path = Pfad
raw-col-value = Wert
raw-group-pd = Persönliche Daten (EF.PD)
raw-group-vd = Versicherungsdaten (EF.VD)
result-footer = Die Daten liegen nur im Arbeitsspeicher und werden verworfen, wenn du sie verwirfst oder die App schließt.

## Persönliche Daten

field-name = Name
field-birth-date = Geburtsdatum
field-sex = Geschlecht
field-insurant-id = Versicherten-ID
field-street = Straße
field-address-addition = Anschriftenzusatz
field-city = Ort
field-country = Land
field-postbox = Postfach
field-postbox-city = Postfach-Ort
sex-male = männlich
sex-female = weiblich
sex-diverse = divers
sex-undetermined = unbestimmt

## Versicherungsdaten

field-payer = Krankenkasse
field-payer-id = IK der Kasse
field-billing-payer = Abrechnende Stelle
field-billing-payer-id = IK abrechnend
field-coverage-start = Versicherungsschutz ab
field-coverage-end = Versicherungsschutz bis
coverage-open-ended = unbefristet
field-insurant-type = Versichertenart
insurant-type-member = Mitglied ({ $code })
insurant-type-family = Familienversicherter ({ $code })
insurant-type-pensioner = Rentner ({ $code })
field-special-group = Besondere Personengruppe
field-dmp = DMP-Kennzeichnung
field-wop = WOP
field-schema = Schemaversion

## Zwischenablage

copy-tooltip = Kopieren
copy-done = Kopiert
copy-all-tooltip = Alles kopieren

## Links in der Fußzeile

footer-github = Quellcode auf GitHub
footer-donate = Spendier mir einen Kaffee

## Export

export-button = Exportieren
export-format-pdf = PDF-Dokument (.pdf)
export-format-text = Textdatei (.txt)
export-format-json = JSON-Daten (.json)
export-dialog-title = Versichertendaten exportieren
export-saved = Gespeichert unter { $path }
export-failed = Export fehlgeschlagen: { $detail }
export-title = Versichertenstammdaten
export-generator = Exportiert mit KnowYourEgk { $version }
export-footer = Dieses Dokument enthält personenbezogene Daten von der elektronischen Gesundheitskarte. Bewahre es sorgfältig auf und gib es nur bewusst weiter.
section-card-status = Kartenstatus
field-last-update = Letzte Aktualisierung auf der Karte
field-version-xml = XML-Schemaversion
field-version-storage = Version des Speicherformats
