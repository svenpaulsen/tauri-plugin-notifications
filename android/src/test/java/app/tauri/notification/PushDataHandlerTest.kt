package app.tauri.notification

import android.content.Context
import com.google.firebase.messaging.RemoteMessage
import org.junit.After
import org.junit.Assert.*
import org.junit.Test

class TestPushDataHandler : PushDataHandler {
    override fun onPushData(context: Context, message: RemoteMessage, appVisible: Boolean) {}
}

class NotAHandler

class PushDataHandlerTest {

    @After
    fun tearDown() {
        PushDataHandlers.resetForTest()
    }

    @Test
    fun testCreate_instantiatesNamedHandler() {
        val handler = PushDataHandlers.create(TestPushDataHandler::class.java.name)
        assertNotNull(handler)
        assertTrue(handler is TestPushDataHandler)
    }

    @Test
    fun testCreate_nullOrBlankNameYieldsNoHandler() {
        assertNull(PushDataHandlers.create(null))
        assertNull(PushDataHandlers.create(""))
        assertNull(PushDataHandlers.create("   "))
    }

    @Test
    fun testCreate_unknownClassIsToleratedNotThrown() {
        assertNull(PushDataHandlers.create("com.example.DoesNotExist"))
    }

    @Test
    fun testCreate_classWithoutInterfaceIsRejected() {
        assertNull(PushDataHandlers.create(NotAHandler::class.java.name))
    }

    @Test
    fun testMetaDataKey() {
        assertEquals("app.tauri.notification.PUSH_DATA_HANDLER", PushDataHandlers.META_DATA_KEY)
    }
}
