package app.tauri.notification

import android.app.ActivityManager
import android.content.Context
import android.content.pm.PackageManager
import android.util.Log
import com.google.firebase.messaging.RemoteMessage

/**
 * App-side hook for FCM messages that carry a `data` payload.
 *
 * A data-only message (no `notification` block) is handed to the app
 * instead of being displayed by the system, and the plugin can only
 * forward it as a `push-message` event to a running WebView. When the
 * process was started by FCM, or the WebView is paused in the background,
 * that event has no receiver and the message would be lost. Apps that
 * render such pushes natively — decrypting an end-to-end encrypted
 * payload, showing an incoming-call notification, … — implement this
 * interface and name the class in their manifest:
 *
 * ```xml
 * <application>
 *   <meta-data
 *     android:name="app.tauri.notification.PUSH_DATA_HANDLER"
 *     android:value="com.example.app.MyPushDataHandler" />
 * </application>
 * ```
 *
 * The class needs a public no-argument constructor and is instantiated
 * once per process. The service still emits the `push-message` event
 * before calling the handler, so a foreground WebView keeps receiving
 * what it receives today; `appVisible` tells the handler whether that
 * event actually reached a screen the user is looking at.
 */
interface PushDataHandler {
  /**
   * Called on the FCM service thread for every message with a non-empty
   * `data` payload — data-only messages and notification messages alike
   * (the latter only reach the service while the app is in the
   * foreground; in the background FCM shows them itself).
   *
   * @param context the service's application context
   * @param message the raw FCM message
   * @param appVisible true when an activity of the app is in the
   *   foreground or visible, i.e. the WebView is live and has already
   *   received the `push-message` event for this message. Handlers
   *   usually show nothing in that case.
   */
  fun onPushData(context: Context, message: RemoteMessage, appVisible: Boolean)
}

/** Resolves and caches the app's [PushDataHandler]. */
object PushDataHandlers {
  const val META_DATA_KEY = "app.tauri.notification.PUSH_DATA_HANDLER"
  private const val TAG = "PushDataHandler"

  private var resolved = false
  private var handler: PushDataHandler? = null

  /** The handler named in the manifest, or null when the app declares
   *  none or the class cannot be instantiated (logged once). */
  @Synchronized
  fun fromManifest(context: Context): PushDataHandler? {
    if (resolved) return handler
    resolved = true
    handler = create(readClassName(context))
    return handler
  }

  /** Instantiates the named handler class. Public for tests; production
   *  code goes through [fromManifest]. */
  fun create(className: String?): PushDataHandler? {
    if (className.isNullOrBlank()) return null
    return try {
      val instance = Class.forName(className).getDeclaredConstructor().newInstance()
      instance as? PushDataHandler ?: run {
        Log.e(TAG, "$className does not implement PushDataHandler")
        null
      }
    } catch (e: Exception) {
      Log.e(TAG, "cannot instantiate push data handler $className", e)
      null
    }
  }

  /** Test-only: forget the cached handler. */
  @Synchronized
  fun resetForTest() {
    resolved = false
    handler = null
  }

  private fun readClassName(context: Context): String? {
    return try {
      val info = context.packageManager.getApplicationInfo(
        context.packageName,
        PackageManager.GET_META_DATA
      )
      info.metaData?.getString(META_DATA_KEY)
    } catch (e: PackageManager.NameNotFoundException) {
      null
    }
  }

  /** Whether one of the app's activities is currently on screen. Reads
   *  the process' own importance, which needs no lifecycle library and
   *  is exact for the single-process case FCM services run in. Only the
   *  two activity-backed levels count: `IMPORTANCE_FOREGROUND` (an
   *  activity has focus) and `IMPORTANCE_VISIBLE` (one is on screen
   *  behind something). `IMPORTANCE_FOREGROUND_SERVICE` sits between them
   *  numerically, so a `<=` comparison would call the app visible while
   *  it merely runs a foreground service — an ongoing voice call with
   *  the app minimised — and its pushes would never be shown. */
  fun isAppVisible(): Boolean {
    val state = ActivityManager.RunningAppProcessInfo()
    ActivityManager.getMyMemoryState(state)
    return state.importance == ActivityManager.RunningAppProcessInfo.IMPORTANCE_FOREGROUND ||
      state.importance == ActivityManager.RunningAppProcessInfo.IMPORTANCE_VISIBLE
  }
}
