import { describe, it, expect } from 'vitest';

describe('Example Test', () => {
  it('should demonstrate basic testing setup', () => {
    expect(2 + 2).toBe(4);
  });

  it('should test string manipulation', () => {
    const text = 'VRChat Photo Uploader';
    expect(text.toLowerCase()).toBe('vrchat photo uploader');
    expect(text.length).toBe(20);
  });

  it('should test array operations', () => {
    const items = ['image1.png', 'image2.jpg', 'image3.gif'];
    expect(items).toHaveLength(3);
    expect(items.filter(item => item.endsWith('.png'))).toHaveLength(1);
  });

  it('should test object properties', () => {
    const webhook = {
      id: 1,
      name: 'Test Webhook',
      url: 'https://discord.com/api/webhooks/123/abc',
      is_forum: false
    };

    expect(webhook).toHaveProperty('id');
    expect(webhook).toHaveProperty('name', 'Test Webhook');
    expect(webhook.is_forum).toBe(false);
  });

  it('should test async operations', async () => {
    const asyncFunction = () => Promise.resolve('Hello, World!');
    const result = await asyncFunction();
    expect(result).toBe('Hello, World!');
  });
});