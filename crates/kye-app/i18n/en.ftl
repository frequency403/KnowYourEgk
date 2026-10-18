# KnowYourEgk UI texts - English (default and fallback language).
# Every message here must also exist in all other .ftl files.

# Dates are stored as YYYYMMDD on the card.
date-format = { $year }-{ $month }-{ $day }
language-label = Language

## Reader panel

readers-heading = Card readers
readers-none = No card reader found.
readers-unavailable = Smart card service (PC/SC) not available: { $detail }
slot-empty = empty
slot-contact = Card inserted (contact)
slot-contactless = Card on reader (contactless)
slot-unknown = Unknown card
button-read = Read
can-needed-hint = Enter the CAN to read contactlessly.
can-label = CAN (6 digits, front of the card, top right):
can-note = The CAN is only needed for contactless reading and is never stored.

## Progress

progress-starting = Starting …
progress-connecting = Connecting to the card …
progress-reading = Reading insurance data …
progress-pace = Establishing secure channel (PACE) with the CAN …
progress-reading-secure = Reading insurance data (encrypted) …
progress-parsing = Evaluating data …

## Result panel

welcome-heading = Welcome
welcome-text = Insert your health card (eGK) into the reader or place it on the reader, then click “Read”.
result-heading = Insurance master data
read-via-contactless = · read contactlessly (PACE)
read-via-contact-pace = · read via contact (PACE)
read-via-contact = · read via contact
button-discard = Discard data
raw-fields = All fields (raw data)
update-incomplete = ⚠ The last update of the insurance data did not complete. The data may be incomplete or outdated.

## Personal data

section-personal = Personal data
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

section-insurance = Insurance data
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

## Errors

error-can-required = The 6-digit CAN is required for contactless reading.
error-wrong-can = The CAN is wrong. Please compare it with the number on the front of the card.
error-card-removed = The card was removed or lost the connection.
error-reader-gone = The card reader is no longer available.
error-no-card = No card in the reader.
error-unsupported = The card does not support a known PACE protocol.
error-service = The smart card service (PC/SC) is not available.
error-card = Card error: { $detail }
