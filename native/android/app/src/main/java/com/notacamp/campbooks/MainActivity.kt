package com.notacamp.campbooks

import android.content.Intent
import android.os.Bundle
import android.view.View
import androidx.activity.enableEdgeToEdge
import dev.hotwire.navigation.activities.HotwireActivity
import dev.hotwire.navigation.navigator.Navigator
import dev.hotwire.navigation.navigator.NavigatorConfiguration
import dev.hotwire.navigation.util.applyDefaultImeWindowInsets

class MainActivity : HotwireActivity() {
    override fun onCreate(savedInstanceState: Bundle?) {
        enableEdgeToEdge()
        super.onCreate(savedInstanceState)

        setContentView(R.layout.activity_main)
        findViewById<View>(R.id.main_nav_host).applyDefaultImeWindowInsets()
    }

    override fun navigatorConfigurations() = listOf(
        NavigatorConfiguration(
            name = "main",
            startLocation = Config.START_LOCATION,
            navigatorHostId = R.id.main_nav_host
        )
    )

    // App is launched cold from the OAuth deep link → handle once the navigator
    // is ready (the web view exists and can navigate).
    override fun onNavigatorReady(navigator: Navigator) {
        super.onNavigatorReady(navigator)
        handleOAuthDeepLink(intent)
    }

    // App was already running (singleTask) → the deep link arrives here.
    override fun onNewIntent(intent: Intent) {
        super.onNewIntent(intent)
        setIntent(intent)
        handleOAuthDeepLink(intent)
    }

    private fun handleOAuthDeepLink(intent: Intent?) {
        val data = intent?.data ?: return
        if (data.scheme != Config.OAUTH_CALLBACK_SCHEME || data.host != "oauth") return

        val navigator = delegate.currentNavigator ?: return
        when (data.getQueryParameter("flow")) {
            "signin" -> {
                val token = data.getQueryParameter("token") ?: return
                // Load in the web view so the session cookie lands in its store.
                navigator.route("${Config.BASE_URL}/session/native?token=$token")
            }
            "connect" -> {
                // Account linked server-side already; reload the accounts screen.
                navigator.route("${Config.BASE_URL}/email_messages?inbox_settings=accounts")
            }
        }

        // Consume so a config change / re-resume doesn't reprocess it.
        intent.data = null
    }
}
