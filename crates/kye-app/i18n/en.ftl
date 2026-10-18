# KnowYourEgk UI texts - English (default and fallback language).
# Every message here must also exist in all other .ftl files.

# Name of this language in this language, shown in the language menu.
language-name = English

# Dates are stored as YYYYMMDD on the card.
date-format = { $year }-{ $month }-{ $day }

## Header

header-tagline = What is stored on your health card
header-offline = Offline · nothing is stored
language-label = Language
theme-label = Appearance
theme-system = System
theme-light = Light
theme-dark = Dark

## Reader sidebar

readers-heading = Card readers
readers-none = No card reader found.
readers-unavailable = Smart card service (PC/SC) not available: { $detail }
readers-footer = Readers can be plugged in and out at any time. A dual-interface reader shows up as two slots.
slot-kind-contact = Contact slot
slot-kind-contactless = Contactless slot (NFC)
slot-kind-unknown = Card reader slot
status-no-card = No card
status-inserted = Card inserted
status-on-reader = Card on the reader
status-unknown-card = Unknown card
can-label = Card access number (CAN)
can-where = 6 digits, top right on the front of your card. Not your PIN.
can-illustration = The CAN is printed at the top right on the front of the card.
can-note = Used once for the encrypted connection, never saved.
button-read = Read card
button-enter-can = Enter the 6-digit CAN
button-reading = Reading …

## Start screen

empty-heading = See what is stored on your health card
empty-text = Insert your eGK into a card reader, or place it on an NFC reader. Then click “Read card” in the reader list.
empty-contact-title = Inserted (contact)
empty-contact-text = Chip first, as marked on the reader. No CAN needed.
empty-nfc-title = On the reader (contactless)
empty-nfc-text = Needs the 6-digit CAN from the front of the card. The connection is encrypted.

## Reading

reading-heading = Reading your card
reading-hint-secure = Contactless, encrypted. This takes about two seconds.
reading-hint-contact = Via the contact slot.
reading-keep = Keep the card in place until your data is shown.
step-connecting = Connecting to the card
step-pace = Secure channel with your CAN (PACE)
step-reading = Reading insurance data
step-reading-secure = Reading insurance data (encrypted)
step-parsing = Evaluating data

## Errors

error-can-required-title = The CAN is missing
error-can-required-text = This card can only be read with its 6-digit card access number (CAN). Enter it in the reader list and try again.
error-wrong-can-title = The CAN does not match
error-wrong-can-text = The card rejected the access number. Compare it with the 6 digits at the top right on the front of the card and try again.
error-card-removed-title = The card was moved
error-card-removed-text = The connection was lost during reading. Put the card back in place and keep it there until your data is shown.
error-reader-gone-title = The card reader is gone
error-reader-gone-text = The reader was unplugged or is no longer available. Reconnect it and try again.
error-no-card-title = No card found
error-no-card-text = There is no card in this reader. Insert the card or place it on the reader.
error-unsupported-title = This card is not supported
error-unsupported-text = The card does not offer a known secure-channel protocol (PACE). KnowYourEgk reads eGK cards of generation G2.1.
error-service-title = Smart card service not available
error-service-text = The smart card service (PC/SC) of your system is not reachable. On Linux, make sure pcscd is running.
error-card-title = Reading failed
error-card-text = The card answered unexpectedly: { $detail }
error-retry-can = Enter CAN again
error-back = Back to start

## Result

update-incomplete-title = The last update of this data did not complete.
update-incomplete-text = The data may be incomplete or outdated. Your doctor's practice can update the card at the next visit.
insurant-id-inline = Insurant ID
via-contactless = Read contactlessly · encrypted
via-contact-pace = Read via contact · encrypted
via-contact = Read via contact
button-discard = Discard data
tab-overview = Overview
tab-personal = Personal data
tab-insurance = Insurance
tab-raw = All fields
section-personal = Personal data
section-insurance = Insurance
raw-intro = Every element of the XML documents stored on the card, including fields without a label in this app.
raw-col-path = Path
raw-col-value = Value
raw-group-pd = Personal data (EF.PD)
raw-group-vd = Insurance data (EF.VD)
result-footer = The data stays in memory only and is dropped when you discard it or close the app.

## Personal data

field-name = Name
field-birth-date = Date of birth
field-sex = Sex
field-insurant-id = Insurant ID
field-street = Street
field-address-addition = Address addition
field-city = City
field-country = Country
field-postbox = Postbox
field-postbox-city = Postbox city
sex-male = male
sex-female = female
sex-diverse = diverse
sex-undetermined = undetermined

## Insurance data

field-payer = Health insurer
field-payer-id = Insurer IK
field-billing-payer = Billing insurer
field-billing-payer-id = Billing IK
field-coverage-start = Coverage from
field-coverage-end = Coverage until
coverage-open-ended = open-ended
field-insurant-type = Insurant type
insurant-type-member = Member ({ $code })
insurant-type-family = Family member ({ $code })
insurant-type-pensioner = Pensioner ({ $code })
field-special-group = Special group
field-dmp = DMP marker
field-wop = WOP
field-schema = Schema version

## Clipboard

copy-tooltip = Copy
copy-done = Copied
copy-all-tooltip = Copy all

## Footer links

footer-github = Source code on GitHub
footer-donate = Buy me a coffee

## Export

export-button = Export
export-format-pdf = PDF document (.pdf)
export-format-text = Text file (.txt)
export-format-json = JSON data (.json)
export-dialog-title = Export insurance data
export-saved = Saved to { $path }
export-failed = Export failed: { $detail }
export-title = Insurance master data
export-generator = Exported with KnowYourEgk { $version }
export-footer = This document contains personal data from the electronic health card. Store and share it carefully.
section-card-status = Card status
field-last-update = Last update on the card
field-version-xml = XML schema version
field-version-storage = Storage format version
