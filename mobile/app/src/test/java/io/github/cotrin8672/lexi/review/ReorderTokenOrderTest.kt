package io.github.cotrin8672.lexi.review

import org.junit.Assert.assertEquals
import org.junit.Assert.assertFalse
import org.junit.Assert.assertTrue
import org.junit.Test

class ReorderTokenOrderTest {
    @Test
    fun bankSlotsKeepFixedPositionsWhenTokensAreSelected() {
        val bank = listOf("the", "team", "adopted", "policy")
        val afterSelect = reorderBankSlots(bank, listOf("team"))
        assertEquals(bank.size, afterSelect.size)
        assertEquals(
            listOf(false, true, false, false),
            afterSelect.map { it.selected },
        )
        assertEquals(
            listOf("the", "adopted", "policy"),
            availableReorderTokens(bank, listOf("team")),
        )
    }

    @Test
    fun bankSlotsRestoreOriginalSlotWhenSelectionIsRemoved() {
        val bank = listOf("the", "team", "adopted", "policy")
        val restored = reorderBankSlots(bank, emptyList())
        assertEquals(bank.size, restored.size)
        assertTrue(restored.all { !it.selected })
        assertEquals(bank, availableReorderTokens(bank, emptyList()))
    }

    @Test
    fun bankSlotsHandleDuplicateWords() {
        val bank = listOf("go", "go", "home")
        val afterOne = reorderBankSlots(bank, listOf("go"))
        assertEquals(3, afterOne.size)
        assertEquals(listOf(true, false, false), afterOne.map { it.selected })

        val afterTwo = reorderBankSlots(bank, listOf("go", "go"))
        assertEquals(listOf(true, true, false), afterTwo.map { it.selected })
        assertEquals(listOf("home"), availableReorderTokens(bank, listOf("go", "go")))
    }

    @Test
    fun selectedSlotsConsumeFromLeftToRightInBankOrder() {
        val bank = listOf("go", "go", "home")
        val slots = reorderBankSlots(bank, listOf("go"))
        assertEquals("go", slots[0].token)
        assertTrue(slots[0].selected)
        assertFalse(slots[1].selected)
    }

    @Test
    fun answerSlotCountMatchesBankTokenCount() {
        val bank = listOf("the", "team", "adopted", "policy")
        assertEquals(bank.size, reorderAnswerSlotCount(bank))
    }
}
