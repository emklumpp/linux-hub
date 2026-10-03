import SwiftUI
import MapKit

struct ContentView: View {
    @Environment(CameraStore.self) private var store
    @Environment(LocationManager.self) private var location

    @State private var position: MapCameraPosition = .region(
        MKCoordinateRegion(center: .init(latitude: 39.74, longitude: -104.99),
                           span: .init(latitudeDelta: 0.6, longitudeDelta: 0.6)))
    @State private var selected: Camera?
    @State private var showFilters = false
    @State private var showNearby = false
    @State private var visibleRegion: MKCoordinateRegion?

    /// Only annotate what is on screen; MapKit chokes on 1,000+ SwiftUI annotations.
    private var visible: [Camera] {
        guard let r = visibleRegion else { return store.filtered }
        let latMin = r.center.latitude - r.span.latitudeDelta / 2, latMax = r.center.latitude + r.span.latitudeDelta / 2
        let lonMin = r.center.longitude - r.span.longitudeDelta / 2, lonMax = r.center.longitude + r.span.longitudeDelta / 2
        let zoomedOut = r.span.latitudeDelta > 0.25
        return store.filtered.filter { c in
            c.lat >= latMin && c.lat <= latMax && c.lon >= lonMin && c.lon <= lonMax
        }.prefix(zoomedOut ? 400 : 1500).map { $0 }
    }

    var body: some View {
        NavigationStack {
            Map(position: $position, selection: $selected) {
                UserAnnotation()
                ForEach(visible) { cam in
                    Annotation(cam.manufacturer ?? "ALPR", coordinate: cam.coordinate, anchor: .center) {
                        CameraPin(camera: cam, isSelected: selected == cam)
                    }
                    .tag(cam)
                    .annotationTitles(.hidden)
                }
                if location.alertsEnabled, let loc = location.current {
                    MapCircle(center: loc.coordinate, radius: location.alertRadius)
                        .foregroundStyle(.orange.opacity(0.12))
                        .stroke(.orange, lineWidth: 1)
                }
            }
            .mapStyle(.standard(elevation: .flat, pointsOfInterest: .excludingAll))
            .mapControls { MapUserLocationButton(); MapCompass() }
            .onMapCameraChange(frequency: .onEnd) { visibleRegion = $0.region }
            .overlay(alignment: .top) { StatusBanner() }
            .sheet(item: $selected) { cam in
                CameraDetailView(camera: cam).presentationDetents([.medium, .large])
            }
            .sheet(isPresented: $showFilters) { FilterView().presentationDetents([.medium]) }
            .sheet(isPresented: $showNearby) { NearbyView(select: { selected = $0 }).presentationDetents([.medium, .large]) }
            .navigationTitle("Denver ALPR cameras")
            .navigationBarTitleDisplayMode(.inline)
            .toolbar {
                ToolbarItemGroup(placement: .topBarLeading) {
                    Button { showFilters = true } label: {
                        Label("Filter", systemImage: store.flockOnly || store.operatorFilter != nil || !store.searchText.isEmpty
                              ? "line.3.horizontal.decrease.circle.fill" : "line.3.horizontal.decrease.circle")
                    }
                }
                ToolbarItemGroup(placement: .topBarTrailing) {
                    Button { showNearby = true } label: { Label("Nearby", systemImage: "location.circle") }
                    Button { Task { await store.refresh() } } label: {
                        if store.isLoading { ProgressView() } else { Label("Refresh", systemImage: "arrow.clockwise") }
                    }.disabled(store.isLoading)
                }
            }
        }
        .onAppear { location.cameraProvider = { [weak store] in store?.filtered ?? [] } }
        .onChange(of: store.flockOnly) { _, _ in location.cameraProvider = { [weak store] in store?.filtered ?? [] } }
    }
}

struct CameraPin: View {
    let camera: Camera
    var isSelected = false
    var body: some View {
        ZStack {
            if let d = camera.direction {
                Image(systemName: "triangle.fill")
                    .font(.system(size: 9))
                    .foregroundStyle(color.opacity(0.6))
                    .offset(y: -11)
                    .rotationEffect(.degrees(d))
            }
            Circle().fill(color)
                .frame(width: isSelected ? 16 : 11, height: isSelected ? 16 : 11)
                .overlay(Circle().stroke(.white, lineWidth: 2))
                .shadow(radius: 1)
        }
        .animation(.snappy, value: isSelected)
    }
    private var color: Color { camera.isFlock ? Color(red: 0.70, green: 0.15, blue: 0.12) : Color(red: 0.17, green: 0.35, blue: 0.66) }
}

/// Count + last-updated pill over the map; also surfaces fetch errors.
struct StatusBanner: View {
    @Environment(CameraStore.self) private var store
    var body: some View {
        VStack(spacing: 6) {
            HStack(spacing: 10) {
                Text("\(store.filtered.count) cameras").fontWeight(.semibold)
                Text("\(store.flockCount) Flock").foregroundStyle(.secondary)
                if let d = store.generatedAt {
                    Text(d, style: .relative).foregroundStyle(.secondary) + Text(" ago").foregroundStyle(.secondary)
                }
            }
            .font(.footnote)
            .padding(.horizontal, 12).padding(.vertical, 7)
            .background(.regularMaterial, in: Capsule())
            if let e = store.errorMessage {
                Text(e).font(.footnote).multilineTextAlignment(.center)
                    .padding(10).background(.red.opacity(0.9), in: RoundedRectangle(cornerRadius: 8))
                    .foregroundStyle(.white).padding(.horizontal)
            }
        }
        .padding(.top, 8)
    }
}
