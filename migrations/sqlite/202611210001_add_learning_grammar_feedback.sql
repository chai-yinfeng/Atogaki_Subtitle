ALTER TABLE local_learning_items
    ADD COLUMN grammar_feedback TEXT CHECK (grammar_feedback IN ('missed', 'wrong'));
