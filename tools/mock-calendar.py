#!/usr/bin/env python3
"""A stand-in for the calendar services, to try Deck Calendar without real calendars.

Serves, on 127.0.0.1:8777, feeds that look like what each service gives out,
with dates relative to today so something is always coming up:

  /apple.ics    iCloud style: IANA zones, a weekly event with an exception and a moved day
  /google.ics   Google style: UTC times, a yearly birthday, a cancelled day
  /outlook.ics  Outlook style: Windows zone names, folded lines, long titles
  /big.ics      a few megabytes of old events, to check the widget copes
  /web.ics      a web page, the way a wrong link answers
  /gone.ics     404
  /slow.ics     the Apple feed after a few seconds

Run it, then give Deck Calendar these addresses as its links (in the testbed,
through --secrets). See the README.
"""
import datetime as dt
import http.server
import sys
import time
from zoneinfo import ZoneInfo

ROME = "Europe/Rome"


def day(offset):
    return dt.date.today() + dt.timedelta(days=offset)


def local(d, hour, minute=0):
    return f"{d:%Y%m%d}T{hour:02d}{minute:02d}00"


def utc(d, hour, minute=0):
    # Rome is two hours ahead of UTC in summer time and one in winter; the
    # tests only need a plausible instant.
    return f"{d:%Y%m%d}T{hour:02d}{minute:02d}00Z"


def fold(line):
    """Folds a content line at 75 bytes, as the standard asks."""
    out, data = [], line.encode()
    while len(data) > 75:
        cut = 75
        while cut > 0 and (data[cut] & 0xC0) == 0x80:
            cut -= 1
        out.append(data[:cut].decode())
        data = b" " + data[cut:]
    out.append(data.decode())
    return "\r\n".join(out)


def calendar(name, events, zones=""):
    lines = ["BEGIN:VCALENDAR", "VERSION:2.0", "PRODID:-//Mock//Deck Calendar//EN", f"X-WR-CALNAME:{name}"]
    lines += zones.split("\r\n") if zones else []
    for ev in events:
        lines += ["BEGIN:VEVENT"] + ev + ["END:VEVENT"]
    lines.append("END:VCALENDAR")
    return "\r\n".join(fold(l) for l in lines if l) + "\r\n"


ROME_ZONE = """BEGIN:VTIMEZONE
TZID:Europe/Rome
BEGIN:DAYLIGHT
DTSTART:19700329T020000
RRULE:FREQ=YEARLY;BYMONTH=3;BYDAY=-1SU
TZOFFSETFROM:+0100
TZOFFSETTO:+0200
END:DAYLIGHT
BEGIN:STANDARD
DTSTART:19701025T030000
RRULE:FREQ=YEARLY;BYMONTH=10;BYDAY=-1SU
TZOFFSETFROM:+0200
TZOFFSETTO:+0100
END:STANDARD
END:VTIMEZONE""".replace("\n", "\r\n")


def apple():
    today = day(0)
    monday = today - dt.timedelta(days=today.weekday())
    now = dt.datetime.now(ZoneInfo(ROME))
    events = [
        ["UID:focus@apple", f"DTSTART;TZID={ROME}:{(now - dt.timedelta(minutes=20)):%Y%m%dT%H%M00}",
         f"DTEND;TZID={ROME}:{(now + dt.timedelta(minutes=40)):%Y%m%dT%H%M00}", "SUMMARY:Focus block: landing page copy", "LOCATION:Desk"],
        [f"UID:standup@apple", f"DTSTART;TZID={ROME}:{local(monday, 9, 30)}", f"DTEND;TZID={ROME}:{local(monday, 9, 45)}",
         "RRULE:FREQ=WEEKLY;BYDAY=MO,TU,WE,TH,FR", f"EXDATE;TZID={ROME}:{local(monday + dt.timedelta(days=1), 9, 30)}",
         "SUMMARY:Team standup", "LOCATION:Zoom"],
        ["UID:dentist@apple", f"DTSTART;TZID={ROME}:{local(day(0), 17, 30)}", f"DTEND;TZID={ROME}:{local(day(0), 18, 30)}",
         "SUMMARY:Dentist", "LOCATION:Via Roma 12, Milano"],
        ["UID:lunch@apple", f"DTSTART;TZID={ROME}:{local(day(0), 13, 0)}", f"DTEND;TZID={ROME}:{local(day(0), 14, 0)}",
         "SUMMARY:Lunch with Marco", "LOCATION:Trattoria da Gino"],
        ["UID:flight@apple", f"DTSTART;VALUE=DATE:{day(3):%Y%m%d}", f"DTEND;VALUE=DATE:{day(6):%Y%m%d}",
         "SUMMARY:Trip to Lisbon"],
        ["UID:gym@apple", f"DTSTART;TZID={ROME}:{local(day(1), 7, 0)}", f"DTEND;TZID={ROME}:{local(day(1), 8, 0)}",
         "RRULE:FREQ=WEEKLY;INTERVAL=2", "SUMMARY:Gym"],
        ["UID:review@apple", f"DTSTART;TZID={ROME}:{local(day(2), 15, 0)}", f"DTEND;TZID={ROME}:{local(day(2), 16, 30)}",
         "SUMMARY:Copy review: the new landing page and its headline options", "LOCATION:Studio"],
    ]
    return calendar("Home", events, ROME_ZONE)


def google():
    events = [
        ["UID:call@google", f"DTSTART:{utc(day(0), 14, 0)}", f"DTEND:{utc(day(0), 14, 45)}", "SUMMARY:Call with Elena",
         "LOCATION:Google Meet"],
        ["UID:bday@google", f"DTSTART;VALUE=DATE:{dt.date(1990, day(4).month, day(4).day):%Y%m%d}",
         f"DTEND;VALUE=DATE:{dt.date(1990, day(4).month, day(4).day) + dt.timedelta(days=1):%Y%m%d}",
         "RRULE:FREQ=YEARLY", "SUMMARY:Giulia's birthday"],
        ["UID:dinner@google", f"DTSTART:{utc(day(1), 18, 30)}", f"DTEND:{utc(day(1), 20, 30)}", "SUMMARY:Dinner at Osteria",
         "LOCATION:Corso Buenos Aires 30"],
        ["UID:yoga@google", f"DTSTART;TZID={ROME}:{local(day(-3), 19, 0)}", f"DTEND;TZID={ROME}:{local(day(-3), 20, 0)}",
         "RRULE:FREQ=WEEKLY;BYDAY=TU,TH", "SUMMARY:Yoga"],
        ["UID:yoga@google", f"RECURRENCE-ID;TZID={ROME}:{local(day(0) + dt.timedelta(days=(1 - day(0).weekday()) % 7), 19, 0)}",
         f"DTSTART;TZID={ROME}:{local(day(0) + dt.timedelta(days=(1 - day(0).weekday()) % 7), 19, 0)}",
         "STATUS:CANCELLED", "SUMMARY:Yoga"],
    ]
    return calendar("Personal", events)


def outlook():
    zone = "W. Europe Standard Time"
    events = [
        ["UID:sync@outlook", f"DTSTART;TZID={zone}:{local(day(0), 11, 0)}", f"DTEND;TZID={zone}:{local(day(0), 12, 0)}",
         "SUMMARY:Quarterly planning sync with the whole marketing and product organisation", "LOCATION:Microsoft Teams Meeting"],
        ["UID:allday@outlook", f"DTSTART;VALUE=DATE:{day(1):%Y%m%d}", f"DTEND;VALUE=DATE:{day(2):%Y%m%d}",
         "SUMMARY:Workshop day"],
        ["UID:1on1@outlook", f"DTSTART;TZID={zone}:{local(day(1), 10, 0)}", f"DTEND;TZID={zone}:{local(day(1), 10, 30)}",
         "RRULE:FREQ=WEEKLY;COUNT=6", "SUMMARY:1:1 with Sam"],
        ["UID:late@outlook", f"DTSTART;TZID={zone}:{local(day(0), 20, 0)}", f"DTEND;TZID={zone}:{local(day(1), 0, 0)}",
         "SUMMARY:Webinar: copy that converts"],
    ]
    return calendar("Work", events)


def big():
    events = []
    base = day(-900)
    for n in range(9000):
        d = base + dt.timedelta(days=n % 880)
        events.append([f"UID:old{n}@big", f"DTSTART:{utc(d, 8 + n % 9, 0)}", f"DTEND:{utc(d, 9 + n % 9, 0)}",
                       f"SUMMARY:Old event number {n} with a reasonably long title to add some bytes",
                       "DESCRIPTION:" + "x" * 240, "LOCATION:Somewhere in the past"])
    events.append(["UID:new@big", f"DTSTART:{utc(day(0), 9, 0)}", f"DTEND:{utc(day(0), 10, 0)}", "SUMMARY:The only event that matters"])
    return calendar("Big", events)


ROUTES = {
    "/apple.ics": (200, "text/calendar", apple),
    "/google.ics": (200, "text/calendar", google),
    "/outlook.ics": (200, "text/calendar", outlook),
    "/big.ics": (200, "text/calendar", big),
    "/web.ics": (200, "text/html", lambda: "<!DOCTYPE html><html><body>Sign in to view this calendar</body></html>"),
    "/gone.ics": (404, "text/plain", lambda: "Not found"),
}


class Handler(http.server.BaseHTTPRequestHandler):
    def do_GET(self):
        path = self.path.split("?")[0]
        if path == "/slow.ics":
            time.sleep(4)
            path = "/apple.ics"
        status, kind, body = ROUTES.get(path, (404, "text/plain", lambda: "Not found"))
        data = body().encode()
        self.send_response(status)
        self.send_header("Content-Type", kind)
        self.send_header("Content-Length", str(len(data)))
        self.end_headers()
        self.wfile.write(data)

    def log_message(self, fmt, *args):
        print(f"{self.address_string()} {fmt % args}", file=sys.stderr)


if __name__ == "__main__":
    server = http.server.ThreadingHTTPServer(("127.0.0.1", 8777), Handler)
    print("Mock calendars on http://127.0.0.1:8777/{apple,google,outlook,big,web,gone,slow}.ics", file=sys.stderr)
    server.serve_forever()
