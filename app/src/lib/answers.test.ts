import assert from 'node:assert/strict';
import { test } from 'node:test';
import {
  NO_CHOICE,
  answerFor,
  assembleAnswers,
  pickOption,
  pickOther,
  readQuestions,
  typeOther,
  type Question,
} from './answers.ts';

const colour: Question = {
  question: 'Which colour?',
  header: 'Colour',
  options: [
    { label: 'Red', description: 'Warm' },
    { label: 'Blue', description: 'Cool' },
  ],
  multiSelect: false,
};

const toppings: Question = {
  question: 'Which toppings?',
  header: 'Toppings',
  options: [
    { label: 'Cheese', description: '' },
    { label: 'Olives', description: '' },
    { label: 'Basil', description: '' },
  ],
  multiSelect: true,
};

test('questions are read from the tool input and malformed entries dropped', () => {
  assert.deepEqual(
    readQuestions({
      questions: [
        { question: 'Which colour?', header: 'Colour', options: [{ label: 'Red', description: 'Warm' }, { label: 'Blue' }, { nope: 1 }], multiSelect: false },
        { header: 'no question text' },
        { question: 'Free?', options: 'none', multiSelect: 'yes' },
      ],
    }),
    [
      { question: 'Which colour?', header: 'Colour', options: [{ label: 'Red', description: 'Warm' }, { label: 'Blue', description: '' }], multiSelect: false },
      { question: 'Free?', header: '', options: [], multiSelect: false },
    ],
  );
  assert.deepEqual(readQuestions(null), []);
  assert.deepEqual(readQuestions({ questions: 'x' }), []);
});

test('a single-select question takes one label or the typed text', () => {
  const red = pickOption(colour, NO_CHOICE, 'Red');
  assert.equal(answerFor(red), 'Red');
  const blue = pickOption(colour, red, 'Blue');
  assert.equal(answerFor(blue), 'Blue');
  const other = pickOther(colour, blue);
  assert.equal(answerFor(other), null);
  assert.equal(answerFor(typeOther(colour, other, '  Green ')), 'Green');
  assert.equal(answerFor(pickOption(colour, typeOther(colour, other, 'Green'), 'Red')), 'Red');
  assert.equal(answerFor(typeOther(colour, red, 'Teal')), 'Teal');
});

test('a multi-select question joins its labels in option order, then the typed text', () => {
  let choice = pickOption(toppings, NO_CHOICE, 'Basil');
  choice = pickOption(toppings, choice, 'Cheese');
  assert.equal(answerFor(choice), 'Cheese, Basil');
  choice = typeOther(toppings, pickOther(toppings, choice), 'Pineapple');
  assert.equal(answerFor(choice), 'Cheese, Basil, Pineapple');
  choice = pickOption(toppings, choice, 'Cheese');
  assert.equal(answerFor(choice), 'Basil, Pineapple');
  choice = pickOther(toppings, choice);
  assert.equal(answerFor(choice), 'Basil');
  assert.equal(answerFor(pickOption(toppings, choice, 'Basil')), null);
});

test('answers are assembled only once every question has one', () => {
  const questions = [colour, toppings];
  assert.equal(assembleAnswers(questions, []), null);
  assert.equal(assembleAnswers(questions, [pickOption(colour, NO_CHOICE, 'Blue')]), null);
  assert.equal(assembleAnswers(questions, [pickOption(colour, NO_CHOICE, 'Blue'), typeOther(toppings, NO_CHOICE, '   ')]), null);
  assert.deepEqual(
    assembleAnswers(questions, [
      pickOption(colour, NO_CHOICE, 'Blue'),
      pickOption(toppings, pickOption(toppings, NO_CHOICE, 'Olives'), 'Cheese'),
    ]),
    { 'Which colour?': 'Blue', 'Which toppings?': 'Cheese, Olives' },
  );
  assert.equal(assembleAnswers([], []), null);
});
