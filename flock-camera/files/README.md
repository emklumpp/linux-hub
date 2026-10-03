# FlockMap (iOS)

SwiftUI + MapKit app showing ALPR / Flock Safety cameras in the Denver metro,
pulled straight from OpenStreetMap's Overpass API (the same crowdsourced data
DeFlock uses). No server required — the Rust/Node project is optional.

Features: map with vendor-colored pins and facing arrows, tap-for-details
(all OSM tags, distance, open in Maps/OSM), Flock-only / operator / tag
filters, "nearest cameras" list, proximity alerts, 24h on-disk cache.

Requires iOS 17+ (uses `@Observable` and the SwiftUI `Map` API).

## Set up in Xcode (5 minutes)

1. Xcode → File → New → Project → iOS **App**.
   Product name `FlockMap`, Interface SwiftUI, Language Swift. Untick Core Data / tests.
2. Delete the generated `ContentView.swift` and `FlockMapApp.swift`.
3. Drag the `FlockMap/` folder from this repo (the one with `Models`, `Services`,
   `Views`, `FlockMapApp.swift`) into the project navigator. Choose
   "Copy items if needed" and add to the FlockMap target.
4. Target → **Info** → add these keys:
   - `NSLocationWhenInUseUsageDescription` — "Shows your position and warns you when a plate reader is nearby."
   - `NSLocationAlwaysAndWhenInUseUsageDescription` — same text (only if you enable background alerts).
5. Optional, for alerts while driving with the app closed:
   Target → **Signing & Capabilities** → + Capability → Background Modes → tick
   **Location updates**, and change `requestWhenInUseAuthorization()` to
   `requestAlwaysAuthorization()` in `LocationManager.swift`.
6. Run on the simulator (Features → Location → Custom Location, e.g. 39.74, -104.99)
   or a device.

## Deploy to your phone / TestFlight

- **Your own device, free**: plug in the phone, pick it as the run destination,
  set Signing → Team to your personal Apple ID. Free-account builds expire
  after 7 days; re-run to renew.
- **TestFlight / App Store**: needs the $99/yr Apple Developer Program.
  Set a unique bundle ID (e.g. `com.yourname.flockmap`), Product → Archive,
  then Distribute App → App Store Connect → Upload. Add testers in
  App Store Connect → TestFlight. For a public App Store listing you'll also
  need screenshots, a privacy policy URL (the app sends no data anywhere
  except Overpass queries), and to declare "Location" in App Privacy.

## Changing the area

Edit `BBox.denverMetro` in `Models/Camera.swift` (south, west, north, east).

## Data notes

Overpass rate-limits; a 429/504 just means wait a minute. Coverage is only
what volunteers have mapped — add missing cameras on openstreetmap.org with
`man_made=surveillance` + `surveillance:type=ALPR` + `manufacturer=Flock Safety`
and they'll show up on the next refresh. Data © OpenStreetMap contributors, ODbL.
