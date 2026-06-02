CREATE TABLE user_genre_restrictions (
    user_id UUID NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    genre   TEXT NOT NULL,
    PRIMARY KEY (user_id, genre)
);
