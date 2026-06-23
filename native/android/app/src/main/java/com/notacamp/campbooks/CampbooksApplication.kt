package com.notacamp.campbooks

import android.app.Application
import dev.hotwire.core.config.Hotwire
import dev.hotwire.core.turbo.config.PathConfiguration
import dev.hotwire.navigation.config.defaultFragmentDestination
import dev.hotwire.navigation.config.registerFragmentDestinations
import dev.hotwire.navigation.config.registerRouteDecisionHandlers
import dev.hotwire.navigation.fragments.HotwireWebFragment
import dev.hotwire.navigation.routing.AppNavigationRouteDecisionHandler
import dev.hotwire.navigation.routing.BrowserTabRouteDecisionHandler
import dev.hotwire.navigation.routing.SystemNavigationRouteDecisionHandler

class CampbooksApplication : Application() {
    override fun onCreate() {
        super.onCreate()

        // Debug builds only: surface the Hotwire visit lifecycle in logcat and
        // enable WebView remote inspection. Disabled in release builds.
        Hotwire.config.debugLoggingEnabled = BuildConfig.DEBUG
        Hotwire.config.webViewDebuggingEnabled = BuildConfig.DEBUG

        // Every screen is a web fragment for now (no native screens yet).
        Hotwire.defaultFragmentDestination = HotwireWebFragment::class
        Hotwire.registerFragmentDestinations(HotwireWebFragment::class)

        // Bundled fallback first, then the live server copy.
        Hotwire.loadPathConfiguration(
            context = this,
            location = PathConfiguration.Location(
                assetFilePath = "json/configuration.json",
                remoteFileUrl = Config.REMOTE_PATH_CONFIGURATION_URL
            )
        )

        // Route OAuth provider URLs to a Custom Tab *before* the default
        // BrowserTab handler claims them. First match wins, so order matters.
        Hotwire.registerRouteDecisionHandlers(
            AppNavigationRouteDecisionHandler(),
            OAuthRouteDecisionHandler(),
            BrowserTabRouteDecisionHandler(),
            SystemNavigationRouteDecisionHandler()
        )
    }
}
