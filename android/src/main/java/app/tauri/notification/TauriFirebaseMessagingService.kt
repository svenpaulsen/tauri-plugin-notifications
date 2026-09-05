package app.tauri.notification

import android.util.Log
import app.tauri.plugin.JSObject
import com.google.firebase.messaging.FirebaseMessagingService
import com.google.firebase.messaging.RemoteMessage

class TauriFirebaseMessagingService : FirebaseMessagingService() {

  override fun onNewToken(token: String) {
    super.onNewToken(token)
    // Store the token for later retrieval and trigger push-token event
    NotificationPlugin.instance?.handleNewToken(token)
  }

  override fun onMessageReceived(message: RemoteMessage) {
    super.onMessageReceived(message)

    // Build push message data from RemoteMessage
    val pushData = mutableMapOf<String, Any>()

    // Add notification data if present
    message.notification?.let { notification ->
      notification.title?.let { pushData["title"] = it }
      notification.body?.let { pushData["body"] = it }
      notification.channelId?.let { pushData["channelId"] = it }
      notification.sound?.let { pushData["sound"] = it }
      notification.tag?.let { pushData["tag"] = it }
    }

    // Add data payload
    if (message.data.isNotEmpty()) {
      pushData["data"] = message.data
    }

    // Add message metadata
    message.messageId?.let { pushData["messageId"] = it }
    message.from?.let { pushData["from"] = it }
    pushData["sentTime"] = message.sentTime

    // Trigger push-message event. `instance` is null when FCM started the
    // process without the WebView — the event is then simply dropped.
    NotificationPlugin.instance?.triggerPushMessage(pushData)

    // Hand the data payload to the app's native handler (see
    // PushDataHandler). This is the only path that reaches the app when
    // the WebView is not running or is paused in the background, so a
    // handler failure must never take the service down with it.
    if (message.data.isNotEmpty()) {
      PushDataHandlers.fromManifest(applicationContext)?.let { handler ->
        try {
          handler.onPushData(applicationContext, message, PushDataHandlers.isAppVisible())
        } catch (e: Exception) {
          Log.e("PushDataHandler", "push data handler failed", e)
        }
      }
    }

    // Also auto-show notification if notification payload exists
    val notification = message.notification
    if (notification != null) {
      val notificationData = Notification().apply {
        id = System.currentTimeMillis().toInt()
        title = notification.title ?: ""
        body = notification.body
        channelId = notification.channelId
        sound = notification.sound

        // Add data payload if available
        if (message.data.isNotEmpty()) {
          val extraData = JSObject()
          for ((key, value) in message.data) {
            extraData.put(key, value)
          }
          extra = extraData
        }
      }

      // Trigger notification event for push notification received in foreground
      NotificationPlugin.triggerNotification(notificationData, "push")
    }
  }
}
