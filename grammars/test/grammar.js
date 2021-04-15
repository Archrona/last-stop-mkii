module.exports = grammar({
    name: 'test',

    extras: $ => [
        /\s/,
        $.line_comment
    ],

    rules: {
        source_file: $ => repeat(choice(
            $.language
        )),

        // Double slash comments accepted anywhere
        line_comment: $ => token(seq('//', /.*/)),

        
        language: $ => seq(
            "language",
            $.identifier,
            "{",
            repeat(choice(
                $.pair
            )),
            "}"
        ),


        pair: $ => seq(
            $.identifier,
            ":",
            $.literal,
            ";"
        ),

        identifier: $ => /[_a-zA-Z]+/,



        literal: $ => choice(
            $.string_literal,
            $.integer_literal,
            $.boolean_literal,
            $.list_literal
        ),

        list_literal: $ => seq(
            "[",
            repeat($.literal),
            "]"
        ),

        integer_literal: $ => /[0-9]+/,

        boolean_literal: $ => choice("true", "false"),

        string_literal: $ => seq(
            '"',
            $.string_content,
            token.immediate('"')
        ),

        string_content: $ => /([^"]|\\\")*/,
    }
});