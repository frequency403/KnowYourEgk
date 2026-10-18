# KnowYourEgk UI-Texte - Deutsch.

# Dates are stored as YYYYMMDD on the card.
date-format = { $day }.{ $month }.{ $year }
language-label = Sprache

## Leserleiste

readers-heading = Kartenleser
readers-none = Kein Kartenleser gefunden.
readers-unavailable = Chipkartendienst (PC/SC) nicht verfügbar: { $detail }
slot-empty = leer
slot-contact = Karte gesteckt (kontaktbehaftet)
slot-contactless = Karte aufgelegt (kontaktlos)
slot-unknown = Unbekannte Karte
button-read = Lesen
can-needed-hint = CAN eingeben, um kontaktlos zu lesen.
can-label = CAN (6 Ziffern, Kartenvorderseite oben rechts):
can-note = Die CAN wird nur für kontaktloses Lesen benötigt und nie gespeichert.

## Fortschritt

progress-starting = Starte …
progress-connecting = Verbinde mit der Karte …
progress-reading = Lese Versichertendaten …
progress-pace = Baue sicheren Kanal (PACE) mit der CAN auf …
progress-reading-secure = Lese Versichertendaten (verschlüsselt) …
progress-parsing = Werte Daten aus …

## Ergebnis

welcome-heading = Willkommen
welcome-text = Stecke die eGK in den Kartenleser oder lege sie auf und klicke auf „Lesen“.
result-heading = Versichertenstammdaten
read-via-contactless = · kontaktlos gelesen (PACE)
read-via-contact-pace = · kontaktbehaftet gelesen (PACE)
read-via-contact = · kontaktbehaftet gelesen
button-discard = Daten verwerfen
raw-fields = Alle Felder (Rohdaten)
update-incomplete = ⚠ Die letzte Aktualisierung der Versichertendaten wurde nicht abgeschlossen. Die Daten können unvollständig oder veraltet sein.

## Persönliche Daten

section-personal = Persönliche Daten
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

section-insurance = Versicherungsdaten
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

## Fehler

error-can-required = Für kontaktloses Lesen wird die 6-stellige CAN benötigt.
error-wrong-can = Die CAN ist falsch. Bitte mit der Nummer auf der Kartenvorderseite vergleichen.
error-card-removed = Die Karte wurde entfernt oder hat die Verbindung verloren.
error-reader-gone = Der Kartenleser ist nicht mehr verfügbar.
error-no-card = Keine Karte im Leser.
error-unsupported = Die Karte unterstützt kein bekanntes PACE-Verfahren.
error-service = Der Chipkartendienst (PC/SC) ist nicht verfügbar.
error-card = Kartenfehler: { $detail }
