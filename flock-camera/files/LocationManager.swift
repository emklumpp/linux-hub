import Foundation
import Observation
import CoreLocation
import UserNotifications

/// Tracks the user's location and fires a local notification when they come
/// within `alertRadius` of a camera. Works in the foreground; enable the
/// "Location updates" background mode in Xcode for background alerts.
@Observable
final class LocationManager: NSObject, CLLocationManagerDelegate {
    var current: CLLocation?
    var authorization: CLAuthorizationStatus = .notDetermined
    var alertsEnabled = false { didSet { alertsEnabled ? start() : stop() } }
    var alertRadius: CLLocationDistance = 250   // meters

    /// Set by ContentView so proximity is checked against the filtered cameras.
    var cameraProvider: () -> [Camera] = { [] }

    private let manager = CLLocationManager()
    private var alerted: Set<Int> = []
    private var lastAlert = Date.distantPast

    override init() {
        super.init()
        manager.delegate = self
        manager.desiredAccuracy = kCLLocationAccuracyNearestTenMeters
        manager.distanceFilter = 25
        manager.pausesLocationUpdatesAutomatically = true
        authorization = manager.authorizationStatus
    }

    func requestPermission() {
        manager.requestWhenInUseAuthorization()
        UNUserNotificationCenter.current().requestAuthorization(options: [.alert, .sound]) { _, _ in }
    }

    func start() {
        if authorization == .notDetermined { requestPermission() }
        manager.startUpdatingLocation()
    }

    func stop() { manager.stopUpdatingLocation(); alerted.removeAll() }

    func locationManagerDidChangeAuthorization(_ m: CLLocationManager) {
        authorization = m.authorizationStatus
        if alertsEnabled, authorization == .authorizedWhenInUse || authorization == .authorizedAlways {
            m.startUpdatingLocation()
        }
    }

    func locationManager(_ m: CLLocationManager, didUpdateLocations locs: [CLLocation]) {
        guard let loc = locs.last else { return }
        current = loc
        guard alertsEnabled else { return }
        checkProximity(at: loc)
    }

    private func checkProximity(at loc: CLLocation) {
        let close = cameraProvider().filter { $0.location.distance(from: loc) <= alertRadius }
        let fresh = close.filter { !alerted.contains($0.id) }
        // Forget cameras once we are clear so they can alert again on the next pass.
        alerted = alerted.filter { id in close.contains { $0.id == id } }
        guard let first = fresh.first, Date().timeIntervalSince(lastAlert) > 20 else { return }
        fresh.forEach { alerted.insert($0.id) }
        lastAlert = Date()

        let content = UNMutableNotificationContent()
        let vendor = first.manufacturer ?? "ALPR"
        content.title = fresh.count > 1 ? "\(fresh.count) plate readers ahead" : "\(vendor) camera ahead"
        content.body = "About \(Int(first.location.distance(from: loc))) m away" + (first.operatorName.map { " · \($0)" } ?? "")
        content.sound = .default
        UNUserNotificationCenter.current().add(
            UNNotificationRequest(identifier: "cam-\(first.id)-\(Int(lastAlert.timeIntervalSince1970))", content: content, trigger: nil))
    }
}
